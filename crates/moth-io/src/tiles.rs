// SPDX-License-Identifier: GPL-3.0-or-later
//! Region-only pyramid output with one-pixel gutters for seamless atlas sampling.
use crate::{DecodeError, Decoded, ImageFormat, MAX_INPUT_BYTES, MAX_PIXELS, Mip};
use image::{ImageDecoder, ImageReader, metadata::Orientation};
use std::{
    io::Cursor,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicBool, Ordering},
};

/// Atlas layers are fixed size, including a filter gutter on each edge.
pub const SIDE: u32 = 256;
/// Gutters prevent bilinear filtering from leaking between adjacent tiles.
pub const CORE: u32 = SIDE - 2;
/// Position in a rebuildable pyramid, independent of native storage or GPU formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tile {
    /// Level zero is the original resolution; each later level halves each axis.
    pub level: u32,
    /// Horizontal tile index, excluding the gutter.
    pub x: u32,
    /// Vertical tile index, excluding the gutter.
    pub y: u32,
}
impl Tile {
    /// Separates tile cache entries from previews without changing board blob identities.
    pub fn key(self, hash: blake3::Hash) -> blake3::Hash {
        let mut key = blake3::Hasher::new_derive_key("MothRef tile cache v1");
        key.update(hash.as_bytes())
            .update(&self.level.to_le_bytes())
            .update(&self.x.to_le_bytes())
            .update(&self.y.to_le_bytes());
        key.finalize()
    }
    /// Every tile extent is clipped against the actual level, including odd dimensions.
    pub fn bounds(self, original: [u32; 2]) -> Option<[u32; 4]> {
        let step = 1u32.checked_shl(self.level)?;
        let size = original.map(|v| v.div_ceil(step));
        let x = self.x.checked_mul(CORE)?;
        let y = self.y.checked_mul(CORE)?;
        (x < size[0] && y < size[1]).then(|| [x, y, CORE.min(size[0] - x), CORE.min(size[1] - y)])
    }
}
/// Decodes only the requested output region; source scanlines are discarded immediately.
/// Decoder panics and cancellation remain values across the worker boundary.
pub fn decode(bytes: &[u8], tile: Tile, cancel: &AtomicBool) -> Result<Decoded, DecodeError> {
    decode_batch(bytes, &[tile], cancel).map(|mut images| images.remove(0))
}
pub(crate) fn decode_batch(
    bytes: &[u8],
    tiles: &[Tile],
    cancel: &AtomicBool,
) -> Result<Vec<Decoded>, DecodeError> {
    if tiles.is_empty() || tiles.len() > 8 || tiles.iter().any(|tile| tile.level != tiles[0].level)
    {
        return Err(DecodeError::Budget);
    }
    catch_unwind(AssertUnwindSafe(|| decode_inner(bytes, tiles, cancel)))
        .map_err(|_| DecodeError::Panic)?
}
fn decode_inner(
    bytes: &[u8],
    tiles: &[Tile],
    cancel: &AtomicBool,
) -> Result<Vec<Decoded>, DecodeError> {
    let _span = tracing::info_span!("decode_tiles", ?tiles, bytes = bytes.len()).entered();
    if cancel.load(Ordering::Relaxed) {
        return Err(DecodeError::Cancelled);
    }
    if bytes.len() as u64 > MAX_INPUT_BYTES || tiles.iter().any(|tile| tile.level > 15) {
        return Err(DecodeError::Budget);
    }
    let raw = bytes.starts_with(crate::RGBA_MAGIC);
    let (format, w, h) = if raw {
        if bytes.len() < 48 {
            return Err(DecodeError::Budget);
        }
        let w = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| DecodeError::Budget)?);
        let h = u32::from_le_bytes(bytes[12..16].try_into().map_err(|_| DecodeError::Budget)?);
        crate::validate_rgba(w, h, bytes.len() - 48)?;
        if blake3::hash(&bytes[48..]).as_bytes() != &bytes[16..48] {
            return Err(DecodeError::Budget);
        }
        (None, w, h)
    } else {
        let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
        let format = reader.format();
        let (w, h) = reader.into_dimensions()?;
        (format, w, h)
    };
    if w == 0 || h == 0 || w > 32768 || h > 32768 {
        return Err(DecodeError::Budget);
    }
    let mut orientation = Orientation::NoTransforms;
    if format == Some(image::ImageFormat::Jpeg) {
        let mut header = jpeg_decoder::Decoder::new(Cursor::new(bytes));
        header.read_info()?;
        let info = header.info().ok_or(DecodeError::Budget)?;
        // Progressive JPEG needs whole-frame coefficient planes even with scanline output.
        if info.coding_process != jpeg_decoder::CodingProcess::DctSequential
            && u64::from(w) * u64::from(h) * 12 > MAX_PIXELS * 4
        {
            return Err(DecodeError::Budget);
        }
        orientation = header
            .exif_data()
            .and_then(Orientation::from_exif_chunk)
            .unwrap_or(orientation);
    } else if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(DecodeError::Budget);
    }
    if format == Some(image::ImageFormat::WebP) {
        if u64::from(w) * u64::from(h) * 4 > crate::MAX_WEBP_FRAME_BYTES
            || bytes.len() as u64 > 8 * 1024 * 1024
        {
            return Err(DecodeError::Budget);
        }
        crate::preview::webp_dimensions(bytes, w, h)?;
        let mut header = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))?;
        if header.has_animation() {
            return Err(DecodeError::Budget);
        }
        orientation = header.orientation()?;
    }
    // Reduced JPEG tiles should not run a full-resolution IDCT and color conversion.
    let jpeg_scale = if format == Some(image::ImageFormat::Jpeg) {
        tiles
            .iter()
            .map(|tile| tile.level)
            .min()
            .unwrap_or(0)
            .min(3)
    } else {
        0
    };
    let source = [w.div_ceil(1 << jpeg_scale), h.div_ceil(1 << jpeg_scale)];
    let mut outputs = tiles
        .iter()
        .map(|tile| {
            Accumulator::new(
                source,
                orientation,
                Tile {
                    level: tile.level - jpeg_scale,
                    ..*tile
                },
                cancel,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let row_end = outputs
        .iter()
        .map(Accumulator::source_rows_end)
        .max()
        .ok_or(DecodeError::Budget)?;
    let mut row_sink = |y, start, step, bytes: &[u8], color| {
        if cancel.load(Ordering::Relaxed) {
            return Err(DecodeError::Cancelled);
        }
        for output in &mut outputs {
            if y < output.source_rows_end() {
                output.row(y, start, step, bytes, color)?;
            }
        }
        Ok(())
    };
    match format {
        Some(image::ImageFormat::Png) => crate::png_rows(bytes, w, h, &mut row_sink)?,
        Some(image::ImageFormat::Gif) => crate::preview::gif_rows(bytes, w, h, &mut row_sink)?,
        Some(image::ImageFormat::Bmp) => crate::preview::bmp_rows(bytes, w, h, &mut row_sink)?,
        Some(image::ImageFormat::Jpeg) => {
            // shortcut: browser JPEG tiles need the Worker codec in M8; previews remain available.
            #[cfg(target_arch = "wasm32")]
            return Err(DecodeError::Budget);
            #[cfg(not(target_arch = "wasm32"))]
            {
                let mut decoder = mozjpeg::Decompress::new_mem(bytes)?;
                if decoder.width() != w as usize || decoder.height() != h as usize {
                    return Err(DecodeError::Budget);
                }
                decoder.scale(8 >> jpeg_scale);
                let cmyk = matches!(
                    decoder.color_space(),
                    mozjpeg::ColorSpace::JCS_CMYK | mozjpeg::ColorSpace::JCS_YCCK
                );
                let mut decoder = if cmyk {
                    decoder.to_colorspace(mozjpeg::ColorSpace::JCS_CMYK)?
                } else {
                    decoder.rgb()?
                };
                let mut row = vec![0u8; source[0] as usize * if cmyk { 4 } else { 3 }];
                let mut rgb = if cmyk {
                    vec![0u8; source[0] as usize * 3]
                } else {
                    Vec::new()
                };
                for y in 0..row_end {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(DecodeError::Cancelled);
                    }
                    decoder.read_scanlines_into(&mut row)?;
                    if cmyk {
                        for (pixel, rgb) in row
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .zip(rgb.as_chunks_mut::<3>().0)
                        {
                            for channel in 0..3 {
                                rgb[channel] = ((u16::from(pixel[channel]) * u16::from(pixel[3])
                                    + 127)
                                    / 255) as u8;
                            }
                        }
                        row_sink(y, 0, 1, &rgb, png::ColorType::Rgb)?;
                    } else {
                        row_sink(y, 0, 1, &row, png::ColorType::Rgb)?;
                    }
                }
                if row_end == source[1] {
                    decoder.finish()?;
                }
                // Destroy an unfinished scanline decoder after the last required source row.
            }
        }
        None if raw => {
            for (y, row) in bytes[48..].chunks_exact(w as usize * 4).enumerate() {
                row_sink(y as u32, 0, 1, row, png::ColorType::Rgba)?;
            }
        }
        Some(image::ImageFormat::WebP) => {
            // WebP's codec exposes a full-frame API; retain the existing strict frame budget.
            if u64::from(w) * u64::from(h) * 4 > crate::MAX_WEBP_FRAME_BYTES {
                return Err(DecodeError::Budget);
            }
            let mut decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(crate::MAX_WEBP_FRAME_BYTES);
            limits.max_image_width = Some(32768);
            limits.max_image_height = Some(32768);
            decoder.set_limits(limits)?;
            let source = image::DynamicImage::from_decoder(decoder)?.into_rgba8();
            for (y, row) in source.as_raw().chunks_exact(w as usize * 4).enumerate() {
                row_sink(y as u32, 0, 1, row, png::ColorType::Rgba)?;
            }
        }
        _ => return Err(DecodeError::Budget),
    }
    let hash = blake3::hash(bytes);
    let original = match orientation {
        Orientation::Rotate90
        | Orientation::Rotate270
        | Orientation::Rotate90FlipH
        | Orientation::Rotate270FlipH => [h, w],
        _ => [w, h],
    };
    Ok(outputs
        .into_iter()
        .zip(tiles)
        .map(|(output, tile)| {
            let mut image = output.finish(hash);
            image.tile = Some(*tile);
            image.original = original;
            image
        })
        .collect())
}
fn oriented(x: u32, y: u32, size: [u32; 2], orientation: Orientation) -> [u32; 2] {
    let [w, h] = size;
    match orientation {
        Orientation::NoTransforms => [x, y],
        Orientation::Rotate90 => [h - 1 - y, x],
        Orientation::Rotate180 => [w - 1 - x, h - 1 - y],
        Orientation::Rotate270 => [y, w - 1 - x],
        Orientation::FlipHorizontal => [w - 1 - x, y],
        Orientation::FlipVertical => [x, h - 1 - y],
        Orientation::Rotate90FlipH => [y, x],
        Orientation::Rotate270FlipH => [h - 1 - y, w - 1 - x],
    }
}
struct Accumulator<'a> {
    source: [u32; 2],
    original: [u32; 2],
    orientation: Orientation,
    tile: Tile,
    bounds: [u32; 4],
    sums: Vec<[u32; 4]>,
    wide_sums: Vec<[u64; 4]>,
    counts: Vec<u32>,
    pixels: Vec<u8>,
    cancel: &'a AtomicBool,
}
impl<'a> Accumulator<'a> {
    fn new(
        source: [u32; 2],
        orientation: Orientation,
        tile: Tile,
        cancel: &'a AtomicBool,
    ) -> Result<Self, DecodeError> {
        let original = match orientation {
            Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH => [source[1], source[0]],
            _ => source,
        };
        let bounds = tile.bounds(original).ok_or(DecodeError::Budget)?;
        Ok(Self {
            source,
            original,
            orientation,
            tile,
            bounds,
            // A level-zero sample already is a pixel; area sums waste nine times its storage.
            // Up to 256² samples per bin fit alpha-weighted color in u32.
            sums: if (1..=8).contains(&tile.level) {
                vec![[0; 4]; (SIDE * SIDE) as usize]
            } else {
                Vec::new()
            },
            wide_sums: if tile.level > 8 {
                vec![[0; 4]; (SIDE * SIDE) as usize]
            } else {
                Vec::new()
            },
            counts: if tile.level == 0 {
                Vec::new()
            } else {
                vec![0; (SIDE * SIDE) as usize]
            },
            pixels: if tile.level == 0 {
                vec![0; (SIDE * SIDE * 4) as usize]
            } else {
                Vec::new()
            },
            cancel,
        })
    }
    fn source_rows_end(&self) -> u32 {
        let axis = usize::from(!matches!(
            self.orientation,
            Orientation::Rotate90
                | Orientation::Rotate270
                | Orientation::Rotate90FlipH
                | Orientation::Rotate270FlipH
        ));
        let first = oriented(0, 0, self.source, self.orientation)[axis];
        let last = oriented(0, self.source[1] - 1, self.source, self.orientation)[axis];
        if first > last {
            self.source[1] - (self.bounds[axis].saturating_sub(1) << self.tile.level)
        } else {
            ((self.bounds[axis] + CORE + 1) << self.tile.level).min(self.source[1])
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
        if self.cancel.load(Ordering::Relaxed) {
            return Err(DecodeError::Cancelled);
        }
        let channels = color.samples();
        if color == png::ColorType::Indexed
            || bytes.len()
                != self.source[0].saturating_sub(start).div_ceil(step) as usize * channels
        {
            return Err(DecodeError::Budget);
        }
        let step_level = 1u32 << self.tile.level;
        let min = [
            self.bounds[0].saturating_sub(1) * step_level,
            self.bounds[1].saturating_sub(1) * step_level,
        ];
        let max = [
            ((self.bounds[0] + CORE + 1) * step_level).min(self.original[0]),
            ((self.bounds[1] + CORE + 1) * step_level).min(self.original[1]),
        ];
        let first = oriented(0, y, self.source, self.orientation);
        let last = oriented(self.source[0] - 1, y, self.source, self.orientation);
        let horizontal = usize::from(matches!(
            self.orientation,
            Orientation::Rotate90
                | Orientation::Rotate270
                | Orientation::Rotate90FlipH
                | Orientation::Rotate270FlipH
        ));
        let other = 1 - horizontal;
        if first[other] < min[other] || first[other] >= max[other] {
            return Ok(());
        }
        let (left, right) = if first[horizontal] <= last[horizontal] {
            (min[horizontal], max[horizontal])
        } else {
            (
                self.source[0] - max[horizontal],
                self.source[0] - min[horizontal],
            )
        };
        let begin = left.saturating_sub(start).div_ceil(step) as usize;
        let end = right
            .saturating_sub(start)
            .div_ceil(step)
            .min(self.source[0].saturating_sub(start).div_ceil(step)) as usize;
        for (i, pixel) in bytes
            .chunks_exact(channels)
            .enumerate()
            .take(end)
            .skip(begin)
        {
            let [x, y] = oriented(start + i as u32 * step, y, self.source, self.orientation)
                .map(|v| v >> self.tile.level);
            let dx = i64::from(x) - i64::from(self.bounds[0]) + 1;
            let dy = i64::from(y) - i64::from(self.bounds[1]) + 1;
            if !(0..i64::from(SIDE)).contains(&dx) || !(0..i64::from(SIDE)).contains(&dy) {
                continue;
            }
            let rgba = match color {
                png::ColorType::Grayscale => [pixel[0], pixel[0], pixel[0], 255],
                png::ColorType::GrayscaleAlpha => [pixel[0], pixel[0], pixel[0], pixel[1]],
                png::ColorType::Rgb => [pixel[0], pixel[1], pixel[2], 255],
                png::ColorType::Rgba => [pixel[0], pixel[1], pixel[2], pixel[3]],
                _ => return Err(DecodeError::Budget),
            };
            let index = (dy as u32 * SIDE + dx as u32) as usize;
            if self.tile.level == 0 {
                self.pixels[index * 4..][..4].copy_from_slice(&if rgba[3] == 0 {
                    [0; 4]
                } else {
                    rgba
                });
                continue;
            }
            if self.tile.level <= 8 {
                for (sum, value) in self.sums[index][..3].iter_mut().zip(rgba[..3].iter()) {
                    *sum += u32::from(*value) * u32::from(rgba[3]);
                }
                self.sums[index][3] += u32::from(rgba[3]);
            } else {
                for (sum, value) in self.wide_sums[index][..3].iter_mut().zip(rgba[..3].iter()) {
                    *sum += u64::from(*value) * u64::from(rgba[3]);
                }
                self.wide_sums[index][3] += u64::from(rgba[3]);
            }
            self.counts[index] += 1;
        }
        Ok(())
    }
    fn finish(self, hash: blake3::Hash) -> Decoded {
        let mut pixels = vec![0; (SIDE * SIDE * 4) as usize];
        let size = self.original.map(|v| v.div_ceil(1 << self.tile.level));
        for y in 0..SIDE {
            for x in 0..SIDE {
                let sx = (i64::from(self.bounds[0]) + i64::from(x) - 1)
                    .clamp(0, i64::from(size[0] - 1))
                    - i64::from(self.bounds[0])
                    + 1;
                let sy = (i64::from(self.bounds[1]) + i64::from(y) - 1)
                    .clamp(0, i64::from(size[1] - 1))
                    - i64::from(self.bounds[1])
                    + 1;
                let index = (sy as u32 * SIDE + sx as u32) as usize;
                if self.tile.level == 0 {
                    pixels[((y * SIDE + x) * 4) as usize..][..4]
                        .copy_from_slice(&self.pixels[index * 4..][..4]);
                    continue;
                }
                let sum = if self.tile.level <= 8 {
                    self.sums[index].map(u64::from)
                } else {
                    self.wide_sums[index]
                };
                let pixel = &mut pixels[((y * SIDE + x) * 4) as usize..][..4];
                for channel in 0..3 {
                    pixel[channel] =
                        (sum[channel] + sum[3] / 2).checked_div(sum[3]).unwrap_or(0) as u8;
                }
                pixel[3] = (sum[3] / u64::from(self.counts[index].max(1))) as u8;
            }
        }
        Decoded {
            tile: Some(self.tile),
            hash,
            original: self.original,
            format: ImageFormat::Rgba8,
            mips: vec![Mip {
                width: SIDE,
                height: SIDE,
                bytes: pixels,
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn reduced_jpeg_tiles_match_scaled_idct_including_odd_edges() {
        let cancel = AtomicBool::new(false);
        for bytes in [
            include_bytes!("../../../corpus/images/4k.jpg").as_slice(),
            include_bytes!("../../../corpus/images/large-dct.jpg").as_slice(),
        ] {
            let original = crate::decode(bytes, 1).unwrap().original;
            for level in 1..=3 {
                let mut decoder = mozjpeg::Decompress::new_mem(bytes).unwrap();
                decoder.scale(8 >> level);
                let mut decoder = decoder.rgb().unwrap();
                let width = decoder.width() as u32;
                let height = decoder.height() as u32;
                let pixels = decoder.read_scanlines::<u8>().unwrap();
                decoder.finish().unwrap();
                for (x, y) in [(0, 0), ((width - 1) / CORE, (height - 1) / CORE)] {
                    let tile = Tile { level, x, y };
                    let image = decode(bytes, tile, &cancel).unwrap();
                    assert_eq!(image.original, original);
                    for py in [0, 1, 127, 254, 255] {
                        for px in [0, 1, 127, 254, 255] {
                            let sx = (i64::from(x * CORE + px) - 1).clamp(0, i64::from(width - 1))
                                as u32;
                            let sy = (i64::from(y * CORE + py) - 1).clamp(0, i64::from(height - 1))
                                as u32;
                            let expected = &pixels[((sy * width + sx) * 3) as usize..][..3];
                            let actual =
                                &image.mips[0].bytes[((py * SIDE + px) * 4) as usize..][..4];
                            assert_eq!(&actual[..3], expected);
                            assert_eq!(actual[3], 255);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn area_bins_preserve_white_without_overflow_at_the_narrow_sum_boundary() {
        let cancel = AtomicBool::new(false);
        for level in [8, 9] {
            let side = 1 << level;
            let mut output = Accumulator::new(
                [side; 2],
                Orientation::NoTransforms,
                Tile { level, x: 0, y: 0 },
                &cancel,
            )
            .unwrap();
            let row = vec![255; side as usize * 4];
            for y in 0..side {
                output.row(y, 0, 1, &row, png::ColorType::Rgba).unwrap();
            }
            let image = output.finish(blake3::hash(b"white"));
            assert!(image.mips[0].bytes.iter().all(|v| *v == 255));
        }
    }
    #[test]
    fn batched_rows_match_separate_decodes_and_keep_the_batch_bounded() {
        let cancel = AtomicBool::new(false);
        let tiles = [
            Tile {
                level: 0,
                x: 0,
                y: 0,
            },
            Tile {
                level: 0,
                x: 0,
                y: 0,
            },
        ];
        for bytes in [
            include_bytes!("../../../corpus/images/rows.png").as_slice(),
            include_bytes!("../../../corpus/images/adam7.png").as_slice(),
            include_bytes!("../../../corpus/images/oriented.jpg").as_slice(),
            include_bytes!("../../../corpus/images/rows.gif").as_slice(),
            include_bytes!("../../../corpus/images/rows.bmp").as_slice(),
            include_bytes!("../../../corpus/images/oriented.webp").as_slice(),
        ] {
            let images = decode_batch(bytes, &tiles, &cancel).unwrap();
            for (image, tile) in images.iter().zip(tiles) {
                let expected = decode(bytes, tile, &cancel).unwrap();
                assert_eq!(image.tile, Some(tile));
                assert_eq!(image.original, expected.original);
                assert_eq!(image.mips[0].bytes, expected.mips[0].bytes);
            }
        }
        assert!(decode_batch(b"", &[], &cancel).is_err());
        assert!(decode_batch(b"", &[tiles[0]; 9], &cancel).is_err());
    }
    #[test]
    fn png_tile_preserves_one_pixel_detail_beyond_2048_and_clips_the_last_column() {
        let source = image::RgbaImage::from_fn(4097, 17, |x, y| {
            image::Rgba([if x % 2 == 0 { 255 } else { 0 }, y as u8, 13, 255])
        });
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(source.clone())
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let cancel = AtomicBool::new(false);
        for tile_x in [9, 16] {
            let tile = Tile {
                level: 0,
                x: tile_x,
                y: 0,
            };
            let decoded = decode(encoded.get_ref(), tile, &cancel).unwrap();
            for y in 0..17 {
                for x in 0..SIDE {
                    let source_x = (i64::from(tile_x * CORE + x) - 1).clamp(0, 4096) as u32;
                    let index = (((y + 1) * SIDE + x) * 4) as usize;
                    assert_eq!(
                        &decoded.mips[0].bytes[index..index + 4],
                        &source.get_pixel(source_x, y).0
                    );
                }
            }
        }
    }
    #[test]
    fn tiles_match_source_pixels_and_gutters_at_every_orientation_and_level() {
        let source = image::RgbaImage::from_fn(521, 317, |x, y| {
            image::Rgba([x as u8, y as u8, 91, if x % 7 == 0 { 0 } else { 255 }])
        });
        for orientation in [
            Orientation::NoTransforms,
            Orientation::Rotate90,
            Orientation::Rotate180,
            Orientation::Rotate270,
            Orientation::FlipHorizontal,
            Orientation::FlipVertical,
            Orientation::Rotate90FlipH,
            Orientation::Rotate270FlipH,
        ] {
            let mut reference = image::DynamicImage::ImageRgba8(source.clone());
            reference.apply_orientation(orientation);
            let reference = reference.into_rgba8();
            for level in 0..=2 {
                let step = 1 << level;
                let size = [
                    reference.width().div_ceil(step),
                    reference.height().div_ceil(step),
                ];
                for y in 0..size[1].div_ceil(CORE) {
                    for x in 0..size[0].div_ceil(CORE) {
                        let tile = Tile { level, x, y };
                        let cancel = AtomicBool::new(false);
                        let mut output = Accumulator::new(
                            [source.width(), source.height()],
                            orientation,
                            tile,
                            &cancel,
                        )
                        .unwrap();
                        let end = output.source_rows_end();
                        for (y, row) in source
                            .as_raw()
                            .chunks_exact(source.width() as usize * 4)
                            .enumerate()
                            .take(end as usize)
                        {
                            output
                                .row(y as u32, 0, 1, row, png::ColorType::Rgba)
                                .unwrap();
                        }
                        let output = output.finish(blake3::hash(b"fixture"));
                        for (px, py) in [(0, 0), (1, 1), (127, 127), (254, 254), (255, 255)] {
                            let sx = (i64::from(x * CORE + px) - 1).clamp(0, i64::from(size[0] - 1))
                                as u32;
                            let sy = (i64::from(y * CORE + py) - 1).clamp(0, i64::from(size[1] - 1))
                                as u32;
                            let mut sum = [0u64; 4];
                            let mut count = 0;
                            for y in sy * step..((sy + 1) * step).min(reference.height()) {
                                for x in sx * step..((sx + 1) * step).min(reference.width()) {
                                    let p = reference.get_pixel(x, y).0;
                                    for c in 0..3 {
                                        sum[c] += u64::from(p[c]) * u64::from(p[3]);
                                    }
                                    sum[3] += u64::from(p[3]);
                                    count += 1;
                                }
                            }
                            let mut expected = [0u8; 4];
                            for c in 0..3 {
                                expected[c] =
                                    (sum[c] + sum[3] / 2).checked_div(sum[3]).unwrap_or(0) as u8;
                            }
                            expected[3] = (sum[3] / count) as u8;
                            let index = ((py * SIDE + px) * 4) as usize;
                            assert_eq!(
                                &output.mips[0].bytes[index..index + 4],
                                &expected,
                                "{orientation:?} {tile:?} ({px},{py})"
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn row_codecs_decode_detail_past_the_preview_ceiling_and_cancel() {
        let cancel = AtomicBool::new(false);
        let tile = Tile {
            level: 0,
            x: 9,
            y: 2,
        };
        for bytes in [
            include_bytes!("../../../corpus/images/4k.jpg").as_slice(),
            include_bytes!("../../../corpus/images/4k.gif").as_slice(),
            include_bytes!("../../../corpus/images/4k.bmp").as_slice(),
        ] {
            let output = decode(bytes, tile, &cancel).unwrap();
            assert_eq!(output.original, [3840, 2160]);
            assert_eq!(output.tile, Some(tile));
            assert_eq!(output.mips[0].bytes.len(), (SIDE * SIDE * 4) as usize);
        }
        cancel.store(true, Ordering::Relaxed);
        assert!(matches!(
            decode(
                include_bytes!("../../../corpus/images/4k.jpg"),
                tile,
                &cancel
            ),
            Err(DecodeError::Cancelled)
        ));
        assert!(
            Tile {
                level: 32,
                x: 0,
                y: 0
            }
            .bounds([3840, 2160])
            .is_none()
        );
        assert!(
            Tile {
                level: 0,
                x: u32::MAX,
                y: 0
            }
            .bounds([3840, 2160])
            .is_none()
        );
    }
    #[test]
    fn png_adam7_and_jpeg_orientation_match_whole_image_decoders() {
        let cancel = AtomicBool::new(false);
        let tile = Tile {
            level: 0,
            x: 0,
            y: 0,
        };
        for bytes in [
            include_bytes!("../../../corpus/images/adam7.png").as_slice(),
            include_bytes!("../../../corpus/images/rows.png").as_slice(),
            include_bytes!("../../../corpus/images/oriented.webp").as_slice(),
            include_bytes!("../../../corpus/images/oriented.jpg").as_slice(),
            include_bytes!("../../../corpus/images/cmyk.jpg").as_slice(),
        ] {
            let preview = crate::decode(bytes, 2048).unwrap();
            let output = decode(bytes, tile, &cancel).unwrap();
            assert_eq!(output.original, preview.original);
            let mip = &preview.mips[0];
            for y in 0..mip.height.min(CORE) {
                for x in 0..mip.width.min(CORE) {
                    let source = ((y * mip.width + x) * 4) as usize;
                    let dest = (((y + 1) * SIDE + x + 1) * 4) as usize;
                    assert!(
                        mip.bytes[source..source + 4]
                            .iter()
                            .zip(&output.mips[0].bytes[dest..dest + 4])
                            .all(|(&a, &b)| a.abs_diff(b) <= 8),
                        "{x},{y}: {:?} != {:?}",
                        &mip.bytes[source..source + 4],
                        &output.mips[0].bytes[dest..dest + 4]
                    );
                }
            }
        }
    }
}
