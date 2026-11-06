// SPDX-License-Identifier: GPL-3.0-or-later
//! Optional reduction of very large images when they are added (off by default).
//!
//! The user asks for it in Settings: the board then keeps a smaller copy instead of
//! the original. Rows stream through one output row of sums at a time, so memory is
//! the reduced image plus a row, never the source frame.
use crate::{DecodeError, tiles};
use image::{DynamicImage, ImageFormat, metadata::Orientation};
use std::sync::atomic::AtomicBool;

/// A reduced copy, encoded for storing as the board's original.
pub struct Reduced {
    /// Encoded bytes: JPEG for JPEG sources, PNG otherwise.
    pub bytes: Vec<u8>,
    /// Size of the reduced copy, after EXIF orientation.
    pub size: [u32; 2],
    /// Size of the source before reduction, after EXIF orientation.
    pub original: [u32; 2],
}

/// Returns a copy whose longer side is `max_side`, or `None` when the image already
/// fits (or is interlaced, which streams out of order): the original is kept then.
pub fn reduce(bytes: &[u8], max_side: u32) -> Result<Option<Reduced>, DecodeError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        reduce_inner(bytes, max_side)
    }))
    .map_err(|_| DecodeError::Panic)?
}

fn reduce_inner(bytes: &[u8], max_side: u32) -> Result<Option<Reduced>, DecodeError> {
    let probe = tiles::probe(bytes)?;
    let [width, height] = probe.size;
    let long = width.max(height);
    if max_side == 0 || long <= max_side {
        return Ok(None);
    }
    let scale = f64::from(max_side) / f64::from(long);
    let size = [
        ((f64::from(width) * scale).round() as u32).max(1),
        ((f64::from(height) * scale).round() as u32).max(1),
    ];
    // JPEG decodes at 1/2, 1/4 or 1/8 for free; take the smallest scale still at least
    // as large as the result, so only the last step is averaging.
    let jpeg_scale = if probe.format == Some(ImageFormat::Jpeg) {
        (0..=3u32)
            .rev()
            .find(|scale| long.div_ceil(1 << scale) >= max_side)
            .unwrap_or(0)
    } else {
        0
    };
    let source = [
        width.div_ceil(1 << jpeg_scale),
        height.div_ceil(1 << jpeg_scale),
    ];
    let mut bands = Bands::new(source, size);
    let cancel = AtomicBool::new(false);
    let mut sink = |y, start, step, row: &[u8], color| bands.row(y, start, step, row, color);
    tiles::scan(bytes, &probe, jpeg_scale, source[1], &cancel, &mut sink)?;
    let Some(image) = bands.finish() else {
        return Ok(None);
    };
    let mut image = DynamicImage::ImageRgba8(image);
    image.apply_orientation(probe.orientation);
    let oriented = |[w, h]: [u32; 2]| match probe.orientation {
        Orientation::Rotate90
        | Orientation::Rotate270
        | Orientation::Rotate90FlipH
        | Orientation::Rotate270FlipH => [h, w],
        _ => [w, h],
    };
    let mut encoded = std::io::Cursor::new(Vec::new());
    if probe.format == Some(ImageFormat::Jpeg) {
        // Photos stay photos: PNG would make a reduced JPEG several times larger.
        let rgb = image.to_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 92).encode_image(&rgb)?;
    } else {
        image.write_to(&mut encoded, ImageFormat::Png)?;
    }
    Ok(Some(Reduced {
        bytes: encoded.into_inner(),
        size: oriented(size),
        original: oriented(probe.size),
    }))
}

/// Area-averaging into horizontal bands: each source row belongs to one output row,
/// each source pixel to one output pixel, colors weighted by alpha.
struct Bands {
    source: [u32; 2],
    size: [u32; 2],
    /// Sums of the output row being filled: alpha-weighted color, then alpha.
    sums: Vec<[u64; 4]>,
    row: Option<u32>,
    last: Option<u32>,
    pixels: Vec<u8>,
    out_of_order: bool,
}

impl Bands {
    fn new(source: [u32; 2], size: [u32; 2]) -> Self {
        Self {
            source,
            size,
            sums: vec![[0; 4]; size[0] as usize],
            row: None,
            last: None,
            pixels: vec![0; size[0] as usize * size[1] as usize * 4],
            out_of_order: false,
        }
    }
    fn row(
        &mut self,
        y: u32,
        start: u32,
        step: u32,
        bytes: &[u8],
        color: png::ColorType,
    ) -> Result<(), DecodeError> {
        if start != 0 || step != 1 || self.last.is_some_and(|last| y <= last) {
            // Interlaced passes revisit rows; the caller keeps the original instead.
            self.out_of_order = true;
            return Ok(());
        }
        self.last = Some(y);
        let out = (u64::from(y) * u64::from(self.size[1]) / u64::from(self.source[1])) as u32;
        if self.row.is_some_and(|row| row != out) {
            self.flush();
        }
        self.row = Some(out);
        let channels = match color {
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            png::ColorType::Indexed => return Err(DecodeError::Budget),
        };
        if bytes.len() != self.source[0] as usize * channels {
            return Err(DecodeError::Budget);
        }
        for (x, pixel) in bytes.chunks_exact(channels).enumerate() {
            let rgba = match *pixel {
                [v] => [v, v, v, 255],
                [v, a] => [v, v, v, a],
                [r, g, b] => [r, g, b, 255],
                [r, g, b, a] => [r, g, b, a],
                _ => return Err(DecodeError::Budget),
            };
            let dx = (x as u64 * u64::from(self.size[0]) / u64::from(self.source[0])) as usize;
            let alpha = u64::from(rgba[3]);
            let sum = &mut self.sums[dx];
            for channel in 0..3 {
                sum[channel] += u64::from(rgba[channel]) * alpha;
            }
            sum[3] += alpha;
        }
        Ok(())
    }
    /// Writes the filled output row; the pixel count of a band follows the same integer
    /// mapping as the accumulation, so edges average exactly what they received.
    fn flush(&mut self) {
        let Some(row) = self.row else {
            return;
        };
        let rows = |r: u64| (r * u64::from(self.source[1])).div_ceil(u64::from(self.size[1]));
        let height = rows(u64::from(row) + 1) - rows(u64::from(row));
        let columns = |c: u64| (c * u64::from(self.source[0])).div_ceil(u64::from(self.size[0]));
        let width = self.size[0] as usize;
        for (x, sum) in self.sums.iter_mut().enumerate() {
            let count = (columns(x as u64 + 1) - columns(x as u64)) * height;
            let at = (row as usize * width + x) * 4;
            for channel in 0..3 {
                self.pixels[at + channel] =
                    (sum[channel] + sum[3] / 2).checked_div(sum[3]).unwrap_or(0) as u8;
            }
            self.pixels[at + 3] = (sum[3] + count / 2).checked_div(count).unwrap_or(0) as u8;
            *sum = [0; 4];
        }
    }
    fn finish(mut self) -> Option<image::RgbaImage> {
        if self.out_of_order {
            return None;
        }
        self.flush();
        image::RgbaImage::from_raw(self.size[0], self.size[1], self.pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_images_shrink_to_the_limit_and_small_ones_are_kept() {
        let jpeg = include_bytes!("../../../corpus/images/large-dct.jpg");
        let reduced = reduce(jpeg, 2048).unwrap().expect("8192 px is above 2048");
        assert_eq!(reduced.original, [8192, 4097]);
        assert_eq!(reduced.size, [2048, 1024]);
        assert!(reduced.bytes.starts_with(&[0xff, 0xd8]), "JPEG stays JPEG");
        let decoded = crate::decode(&reduced.bytes, 64).unwrap();
        assert_eq!(decoded.original, [2048, 1024]);
        assert!(
            reduce(jpeg, 8192).unwrap().is_none(),
            "fits: keep the original"
        );
        let png = include_bytes!("../../../corpus/images/rows.png");
        let size = crate::tiles::dimensions(png).unwrap();
        let reduced = reduce(png, size[0].max(size[1]) / 2).unwrap();
        if let Some(reduced) = reduced {
            assert!(reduced.bytes.starts_with(b"\x89PNG"));
        }
    }
    #[test]
    fn bands_average_whole_blocks() {
        let mut bands = Bands::new([4, 2], [2, 1]);
        for y in 0..2 {
            bands
                .row(
                    y,
                    0,
                    1,
                    &[0, 0, 0, 255, 255, 255, 0, 0, 0, 255, 255, 255],
                    png::ColorType::Rgb,
                )
                .unwrap();
        }
        let image = bands.finish().unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [128, 128, 128, 255]);
    }
}
