// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Bounded image workers. Pixels never enter the scene model.
mod bake;
pub mod cache;
mod pack;
mod preview;
mod scheduler;
pub mod tiles;
pub use bake::BakeMode;
pub use cache::{CacheMode, LodCache, LodStore, LodWriter};
use crossbeam_channel::Receiver;
use image::{DynamicImage, ImageReader, metadata::Orientation};
use moth_format::{Handle, Storage};
pub use scheduler::Requests;
use std::{
    io::{Read, Write},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, atomic::Ordering},
};

/// Prevents compressed input from exhausting memory before header validation.
pub const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
/// Bounds transient decoding allocations, including malicious image headers.
pub const MAX_PIXELS: u64 = 32 * 1024 * 1024;
/// Largest whole-image preview; visible tiles provide detail beyond this ceiling.
pub const MAX_SIDE: u32 = 2048;
/// WebP has no reduced-output API; bound the permitted full RGBA frame to 8 MiB.
pub const MAX_WEBP_FRAME_BYTES: u64 = 8 * 1024 * 1024;
/// GPU texture format for the decoded mips, chosen at runtime from the adapter.
///
/// `Rgba8` is the portable baseline; `Bc7` (desktop) halves texture memory for
/// the same residency. ASTC is the future mobile format and is added with M8.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ImageFormat {
    /// Uncompressed RGBA8: universally supported, including software adapters.
    #[default]
    Rgba8,
    /// BC7 blocks; encoding is x86/x86_64 only, so non-x86 targets never request it.
    Bc7,
}
/// A single rebuildable LOD, suitable for one array layer.
pub struct Mip {
    /// Positive extents preserve valid geometry and aspect ratio.
    pub width: u32,
    /// Positive extents preserve valid geometry and aspect ratio.
    pub height: u32,
    /// RGBA pixels, or BC7 blocks when the parent [`Decoded`] is `Bc7`.
    pub bytes: Vec<u8>,
}
/// Original identity survives changes in preview resolution or GPU encoding.
pub struct Decoded {
    /// A tile carries the original identity but occupies a fixed-size atlas layer.
    pub tile: Option<tiles::Tile>,
    /// Deduplicates original content independently of preview resolution.
    pub hash: blake3::Hash,
    /// Preserves aspect ratio without retaining full-resolution pixels.
    pub original: [u32; 2],
    /// Lower levels let the renderer avoid oversized image samples.
    pub mips: Vec<Mip>,
    /// Layout of each [`Mip::bytes`] payload.
    pub format: ImageFormat,
}
/// Identifies a requested preview without giving core code a native path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    /// Missing detail is requested independently for each visible tile.
    pub tile: Option<tiles::Tile>,
    /// Opaque identity keeps native paths out of core crates.
    pub handle: Handle,
    /// Bounds preview work independently of source dimensions.
    pub side: u32,
    /// Target GPU format; the worker re-encodes decoded mips to this on success.
    pub format: ImageFormat,
    /// Content hash when already known (previews of board items). It lets the worker
    /// answer from the [`LodCache`] without reading the original; imports pass `None`.
    pub blob: Option<blake3::Hash>,
}
/// Failures become UI placeholders, including decoder panics.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("storage: {0}")]
    /// Preserves the cause of storage failures.
    Io(#[from] std::io::Error),
    #[error("image: {0}")]
    /// Keeps image-library failures inside the decode boundary.
    Image(#[from] image::ImageError),
    #[error("image exceeds the input or dimension budget")]
    /// Rejects untrusted allocations beyond the configured limit.
    Budget,
    #[error("JPEG: {0}")]
    /// Preserves JPEG diagnostics without crashing a worker.
    Jpeg(#[from] jpeg_decoder::Error),
    #[error("PNG: {0}")]
    /// Preserves PNG diagnostics without crashing a worker.
    Png(#[from] png::DecodingError),
    /// Clipboard originals use the existing lossless PNG encoder.
    #[error("PNG encoding: {0}")]
    PngEncoding(#[from] png::EncodingError),
    #[error("decoder panicked")]
    /// Converts a decoder panic into a recoverable failure.
    Panic,
    /// Superseded viewport work does not become a broken-image placeholder.
    #[error("request cancelled")]
    Cancelled,
    /// GIF failures remain inside the worker error boundary.
    #[error("GIF: {0}")]
    Gif(#[from] gif::DecodingError),
}
/// Header checks precede pixel allocation, even for JPEG's scaled path.
pub fn decode(bytes: &[u8], side: u32) -> Result<Decoded, DecodeError> {
    catch_unwind(AssertUnwindSafe(|| decode_inner(bytes, side))).map_err(|_| DecodeError::Panic)?
}
fn decode_inner(bytes: &[u8], side: u32) -> Result<Decoded, DecodeError> {
    let _span = tracing::info_span!("decode", bytes = bytes.len(), side).entered();
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(DecodeError::Budget);
    }
    if bytes.starts_with(RGBA_MAGIC) {
        return decode_rgba(bytes, side.clamp(1, MAX_SIDE));
    }
    let mut reader = ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut header_limits = image::Limits::default();
    header_limits.max_alloc = Some(8 * 1024 * 1024);
    header_limits.max_image_width = Some(32768);
    header_limits.max_image_height = Some(32768);
    reader.limits(header_limits);
    let format = reader.format();
    let (w, h) = reader.into_dimensions()?;
    if w == 0
        || h == 0
        || w > 32768
        || h > 32768
        || (format != Some(image::ImageFormat::Jpeg) && u64::from(w) * u64::from(h) > MAX_PIXELS)
    {
        return Err(DecodeError::Budget);
    }
    if format == Some(image::ImageFormat::WebP)
        && (u64::from(w) * u64::from(h) * 4 > MAX_WEBP_FRAME_BYTES
            || bytes.len() as u64 > 8 * 1024 * 1024)
    {
        return Err(DecodeError::Budget);
    }
    let side = side.clamp(1, MAX_SIDE);
    let mut orientation = Orientation::NoTransforms;
    let mut image = if format == Some(image::ImageFormat::Jpeg) {
        let mut jpeg = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
        let divisor = [1u32, 2, 4, 8]
            .into_iter()
            .find(|d| w.max(h).div_ceil(*d) <= side)
            .unwrap_or(8);
        let (scaled_w, scaled_h) =
            jpeg.scale(w.div_ceil(divisor) as u16, h.div_ceil(divisor) as u16)?;
        let info = jpeg.info().ok_or(DecodeError::Budget)?;
        let channels = info.pixel_format.pixel_bytes() as u64;
        let mut allocation = u64::from(scaled_w) * u64::from(scaled_h) * channels * 2;
        // Progressive scans retain source-sized coefficient planes even after DCT scaling.
        if info.coding_process == jpeg_decoder::CodingProcess::DctProgressive {
            allocation +=
                u64::from(w.div_ceil(8) * 8) * u64::from(h.div_ceil(8) * 8) * channels * 2;
        }
        if allocation > MAX_PIXELS * 4 {
            return Err(DecodeError::Budget);
        }
        jpeg.set_max_decoding_buffer_size((MAX_PIXELS * 4) as usize);
        let pixels = jpeg.decode()?;
        orientation = jpeg
            .exif_data()
            .and_then(Orientation::from_exif_chunk)
            .unwrap_or(Orientation::NoTransforms);
        let info = jpeg.info().ok_or(DecodeError::Budget)?;
        match info.pixel_format {
            jpeg_decoder::PixelFormat::RGB24 => DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(info.width.into(), info.height.into(), pixels)
                    .ok_or(DecodeError::Budget)?,
            ),
            jpeg_decoder::PixelFormat::L8 => DynamicImage::ImageLuma8(
                image::GrayImage::from_raw(info.width.into(), info.height.into(), pixels)
                    .ok_or(DecodeError::Budget)?,
            ),
            jpeg_decoder::PixelFormat::CMYK32 => {
                let mut rgb = Vec::with_capacity(pixels.len() / 4 * 3);
                for pixel in pixels.as_chunks::<4>().0 {
                    for &ink in &pixel[..3] {
                        rgb.push(
                            ((u16::from(255 - ink) * u16::from(255 - pixel[3]) + 127) / 255) as u8,
                        );
                    }
                }
                DynamicImage::ImageRgb8(
                    image::RgbImage::from_raw(info.width.into(), info.height.into(), rgb)
                        .ok_or(DecodeError::Budget)?,
                )
            }
            _ => return Err(DecodeError::Budget),
        }
    } else if format == Some(image::ImageFormat::Png) {
        DynamicImage::ImageRgba8(png_preview(bytes, w, h, side)?)
    } else if format == Some(image::ImageFormat::Gif) {
        DynamicImage::ImageRgba8(preview::gif(bytes, w, h, side)?)
    } else if format == Some(image::ImageFormat::Bmp) {
        DynamicImage::ImageRgba8(preview::bmp(bytes, w, h, side)?)
    } else if format == Some(image::ImageFormat::WebP) {
        preview::webp_dimensions(bytes, w, h)?;
        use image::ImageDecoder;
        let mut decoder = image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(bytes))?;
        if decoder.has_animation() {
            return Err(DecodeError::Budget);
        }
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(MAX_WEBP_FRAME_BYTES);
        limits.max_image_width = Some(32768);
        limits.max_image_height = Some(32768);
        decoder.set_limits(limits)?;
        orientation = decoder.orientation()?;
        let source = DynamicImage::from_decoder(decoder)?.into_rgba8();
        DynamicImage::ImageRgba8(preview::rgba(source.as_raw(), w, h, side)?)
    } else {
        return Err(DecodeError::Budget);
    };
    image.apply_orientation(orientation);
    let original = match orientation {
        Orientation::Rotate90
        | Orientation::Rotate270
        | Orientation::Rotate90FlipH
        | Orientation::Rotate270FlipH => [h, w],
        _ => [w, h],
    };
    let image = if image.width().max(image.height()) > side {
        image.thumbnail(side, side)
    } else {
        image
    };
    Ok(pyramid(image.into_rgba8(), blake3::hash(bytes), original))
}
fn pyramid(mut level: image::RgbaImage, hash: blake3::Hash, original: [u32; 2]) -> Decoded {
    let mut mips = Vec::new();
    loop {
        let (width, height) = level.dimensions();
        let next = if width > 1 || height > 1 {
            // Integer area averaging avoids a full-frame f32 intermediate for every mip.
            Some(image::imageops::thumbnail(
                &level,
                (width / 2).max(1),
                (height / 2).max(1),
            ))
        } else {
            None
        };
        mips.push(Mip {
            width,
            height,
            bytes: level.into_raw(),
        });
        match next {
            Some(n) => level = n,
            None => break,
        }
    }
    Decoded {
        tile: None,
        hash,
        original,
        mips,
        format: ImageFormat::Rgba8,
    }
}
/// Re-encodes decoded mips to the requested format; `Rgba8` is a no-op.
///
/// BC7 encoding is compiled only on x86/x86_64 (the encoder's kernels are ISPC).
/// `moth_render::pick_image_format` never requests BC7 on non-x86 builds, so the
/// `Bc7` arm is unreachable there.
fn apply_format(format: ImageFormat, image: Decoded) -> Decoded {
    match format {
        ImageFormat::Bc7 => {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            {
                to_bc7(image)
            }
            #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
            {
                image
            }
        }
        ImageFormat::Rgba8 => image,
    }
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn to_bc7(mut image: Decoded) -> Decoded {
    image.format = ImageFormat::Bc7;
    // BC7 blocks are 4x4; drop mips a compressed texture level cannot represent.
    image.mips.retain(|m| m.width >= 4 && m.height >= 4);
    for mip in &mut image.mips {
        mip.bytes = encode_bc7(&mip.bytes, mip.width, mip.height);
    }
    image
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn encode_bc7(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    // Replicate edge texels out to the 4x4 block grid so partial blocks have no seams.
    let pw = w.next_multiple_of(4);
    let ph = h.next_multiple_of(4);
    let mut padded = Vec::with_capacity(pw * ph * 4);
    for y in 0..ph {
        let row = &rgba[y.min(h - 1) * w * 4..][..w * 4];
        padded.extend_from_slice(row);
        for _ in w..pw {
            padded.extend_from_slice(&row[(w - 1) * 4..]);
        }
    }
    intel_tex_2::bc7::compress_blocks(
        &intel_tex_2::bc7::alpha_very_fast_settings(),
        &intel_tex_2::RgbaSurface {
            data: &padded,
            width: pw as u32,
            height: ph as u32,
            stride: (pw * 4) as u32,
        },
    )
}
const RGBA_MAGIC: &[u8; 8] = b"MOTHRG01";
/// Writes original clipboard pixels without a PNG encode/decode round-trip.
/// The versioned scratch record is checksummed; it is not a `.moth` board file.
pub fn write_rgba(
    mut writer: impl Write,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), DecodeError> {
    validate_rgba(width, height, rgba.len())?;
    writer.write_all(RGBA_MAGIC)?;
    writer.write_all(&width.to_le_bytes())?;
    writer.write_all(&height.to_le_bytes())?;
    writer.write_all(blake3::hash(rgba).as_bytes())?;
    writer.write_all(rgba)?;
    Ok(())
}
/// Compresses clipboard originals losslessly and hashes the encoded bytes as they are written.
pub fn write_png(
    writer: impl Write,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<blake3::Hash, DecodeError> {
    validate_rgba(width, height, rgba.len())?;
    let mut output = HashWriter {
        writer,
        hash: blake3::Hasher::new(),
    };
    let mut encoder = png::Encoder::new(&mut output, width, height);
    let opaque = rgba.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255);
    encoder.set_color(if opaque {
        png::ColorType::Rgb
    } else {
        png::ColorType::Rgba
    });
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fastest);
    let mut writer = encoder.write_header()?;
    {
        let mut stream = writer.stream_writer_with_size(256 * 1024)?;
        if opaque {
            let mut row = vec![0; width as usize * 3];
            for pixels in rgba.chunks_exact(width as usize * 4) {
                for (rgb, pixel) in row
                    .as_chunks_mut::<3>()
                    .0
                    .iter_mut()
                    .zip(pixels.as_chunks::<4>().0)
                {
                    rgb.copy_from_slice(&pixel[..3]);
                }
                stream.write_all(&row)?;
            }
        } else {
            stream.write_all(rgba)?;
        }
        stream.finish()?;
    }
    writer.finish()?;
    Ok(output.hash.finalize())
}
struct HashWriter<W> {
    writer: W,
    hash: blake3::Hasher,
}
impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let written = self.writer.write(bytes)?;
        self.hash.update(&bytes[..written]);
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}
fn validate_rgba(width: u32, height: u32, len: usize) -> Result<(), DecodeError> {
    if width == 0
        || height == 0
        || width > 32768
        || height > 32768
        || u64::from(width) * u64::from(height) > MAX_PIXELS
        || u64::from(width) * u64::from(height) * 4 != len as u64
        || len as u64 + 48 > MAX_INPUT_BYTES
    {
        return Err(DecodeError::Budget);
    }
    Ok(())
}
fn decode_rgba(bytes: &[u8], side: u32) -> Result<Decoded, DecodeError> {
    if bytes.len() < 48 {
        return Err(DecodeError::Budget);
    }
    let width = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| DecodeError::Budget)?);
    let height = u32::from_le_bytes(bytes[12..16].try_into().map_err(|_| DecodeError::Budget)?);
    let pixels = &bytes[48..];
    validate_rgba(width, height, pixels.len())?;
    if blake3::hash(pixels).as_bytes() != &bytes[16..48] {
        return Err(DecodeError::Budget);
    }
    rgba_preview_inner(width, height, pixels, side, blake3::hash(bytes))
}
/// Builds a clipboard preview directly from the already transferred pixels.
/// Its identity exactly matches `write_rgba`, so eviction can reload the durable record.
pub fn preview_rgba(
    width: u32,
    height: u32,
    pixels: &[u8],
    side: u32,
) -> Result<Decoded, DecodeError> {
    validate_rgba(width, height, pixels.len())?;
    let checksum = blake3::hash(pixels);
    let mut hash = blake3::Hasher::new();
    hash.update(RGBA_MAGIC)
        .update(&width.to_le_bytes())
        .update(&height.to_le_bytes())
        .update(checksum.as_bytes())
        .update(pixels);
    rgba_preview_inner(
        width,
        height,
        pixels,
        side.clamp(1, MAX_SIDE),
        hash.finalize(),
    )
}
/// Builds a preview without hashing pixels when the persisted original's identity is known.
pub fn preview_rgba_with_hash(
    width: u32,
    height: u32,
    pixels: &[u8],
    side: u32,
    hash: blake3::Hash,
) -> Result<Decoded, DecodeError> {
    validate_rgba(width, height, pixels.len())?;
    rgba_preview_inner(width, height, pixels, side.clamp(1, MAX_SIDE), hash)
}
fn rgba_preview_inner(
    width: u32,
    height: u32,
    pixels: &[u8],
    side: u32,
    hash: blake3::Hash,
) -> Result<Decoded, DecodeError> {
    let ratio = (side as f64 / f64::from(width.max(height))).min(1.);
    let w = (f64::from(width) * ratio).round().max(1.) as u32;
    let h = (f64::from(height) * ratio).round().max(1.) as u32;
    if w == width && h == height {
        return Ok(pyramid(
            image::RgbaImage::from_raw(w, h, pixels.to_vec()).ok_or(DecodeError::Budget)?,
            hash,
            [width, height],
        ));
    }
    let mut average = AreaPreview::new(width, height, w, h);
    for (y, row) in pixels.chunks_exact((width * 4) as usize).enumerate() {
        average.row(y as u32, 0, 1, row, png::ColorType::Rgba)?;
    }
    Ok(pyramid(average.finish(), hash, [width, height]))
}
// Alpha-weighted area bins keep thin features and transparent edges without retaining source pixels.
struct AreaPreview {
    source: [u32; 2],
    size: [u32; 2],
    sums: Vec<[u64; 4]>,
    pixels: Vec<u8>,
}
impl AreaPreview {
    fn new(width: u32, height: u32, w: u32, h: u32) -> Self {
        Self {
            source: [width, height],
            size: [w, h],
            sums: if [width, height] == [w, h] {
                Vec::new()
            } else {
                vec![[0; 4]; (w * h) as usize]
            },
            pixels: if [width, height] == [w, h] {
                vec![0; (w * h * 4) as usize]
            } else {
                Vec::new()
            },
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
        // Specialize the hot loop once per row, so channel counts and opaque alpha are constants.
        match color {
            png::ColorType::Grayscale => {
                self.row_pixels(y, start, step, bytes, |[v]| [v, v, v, 255])
            }
            png::ColorType::GrayscaleAlpha => {
                self.row_pixels(y, start, step, bytes, |[v, a]| [v, v, v, a])
            }
            png::ColorType::Rgb => {
                self.row_pixels(y, start, step, bytes, |[r, g, b]| [r, g, b, 255])
            }
            png::ColorType::Rgba => self.row_pixels(y, start, step, bytes, |p| p),
            png::ColorType::Indexed => Err(DecodeError::Budget),
        }
    }
    fn row_pixels<const N: usize>(
        &mut self,
        y: u32,
        start: u32,
        step: u32,
        bytes: &[u8],
        rgba: impl Fn([u8; N]) -> [u8; 4],
    ) -> Result<(), DecodeError> {
        let offset = (y * self.size[1] / self.source[1] * self.size[0]) as usize;
        if bytes.len() != ((self.source[0] - start).div_ceil(step) as usize) * N {
            return Err(DecodeError::Budget);
        }
        let pixels = bytes.as_chunks::<N>().0;
        if !self.pixels.is_empty() {
            for (index, &pixel) in pixels.iter().enumerate() {
                let rgba = rgba(pixel);
                let x = start as usize + index * step as usize;
                self.pixels[(offset + x) * 4..][..4].copy_from_slice(&if rgba[3] == 0 {
                    [0; 4]
                } else {
                    rgba
                });
            }
            return Ok(());
        }
        let mut consumed = 0;
        for dx in 0..self.size[0] {
            let end = ((dx + 1) * self.source[0]).div_ceil(self.size[0]);
            let next = end.saturating_sub(start).div_ceil(step) as usize;
            let mut sum = self.sums[offset + dx as usize];
            for &p in &pixels[consumed..next] {
                let rgba = rgba(p);
                let alpha = u64::from(rgba[3]);
                for channel in 0..3 {
                    sum[channel] += u64::from(rgba[channel]) * alpha;
                }
                sum[3] += alpha;
            }
            self.sums[offset + dx as usize] = sum;
            consumed = next;
        }
        Ok(())
    }
    fn finish(self) -> image::RgbaImage {
        let [w, h] = self.size;
        let [width, height] = self.source;
        if !self.pixels.is_empty() {
            return image::RgbaImage::from_raw(w, h, self.pixels)
                .expect("direct preview allocates exactly one RGBA pixel per output coordinate");
        }
        let mut output = image::RgbaImage::new(w, h);
        for (index, pixel) in output.pixels_mut().enumerate() {
            let x = index as u32 % w;
            let y = index as u32 / w;
            let count = u64::from(((x + 1) * width).div_ceil(w) - (x * width).div_ceil(w))
                * u64::from(((y + 1) * height).div_ceil(h) - (y * height).div_ceil(h));
            let sum = self.sums[index];
            for channel in 0..3 {
                pixel[channel] = (sum[channel] + sum[3] / 2).checked_div(sum[3]).unwrap_or(0) as u8;
            }
            pixel[3] = ((sum[3] + count / 2) / count) as u8;
        }
        output
    }
}

// Stream PNG rows into the requested LOD, including Adam7 passes; never allocate the source frame.
fn png_preview(
    bytes: &[u8],
    width: u32,
    height: u32,
    side: u32,
) -> Result<image::RgbaImage, DecodeError> {
    let ratio = (side as f64 / f64::from(width.max(height))).min(1.);
    let w = (f64::from(width) * ratio).round().max(1.) as u32;
    let h = (f64::from(height) * ratio).round().max(1.) as u32;
    let mut output = AreaPreview::new(width, height, w, h);
    png_rows(bytes, width, height, &mut |y, start, step, bytes, color| {
        output.row(y, start, step, bytes, color)
    })?;
    Ok(output.finish())
}
fn png_rows(
    bytes: &[u8],
    width: u32,
    height: u32,
    row_sink: &mut impl FnMut(u32, u32, u32, &[u8], png::ColorType) -> Result<(), DecodeError>,
) -> Result<(), DecodeError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: 8 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let (color, depth) = reader.output_color_type();
    if depth != png::BitDepth::Eight {
        return Err(DecodeError::Budget);
    }
    let passes: &[(u32, u32, u32, u32)] = if reader.info().interlaced {
        &[
            (0, 0, 8, 8),
            (4, 0, 8, 8),
            (0, 4, 4, 8),
            (2, 0, 4, 4),
            (0, 2, 2, 4),
            (1, 0, 2, 2),
            (0, 1, 1, 2),
        ]
    } else {
        &[(0, 0, 1, 1)]
    };
    for &(x_start, y_start, x_step, y_step) in passes {
        if x_start >= width || y_start >= height {
            continue;
        }
        for y in (y_start..height).step_by(y_step as usize) {
            let row = reader.next_row()?.ok_or(DecodeError::Budget)?;
            row_sink(y, x_start, x_step, row.data(), color)?;
        }
    }
    reader.finish()?;
    Ok(())
}
/// Results are bounded too: a slow frame cannot accumulate decoded images.
pub struct Completion {
    /// A corrupt source still has a content identity for its error placeholder.
    pub failure_hash: Option<blake3::Hash>,
    /// Correlates out-of-order worker completions.
    pub request: Request,
    /// Carries failures as values across the worker boundary.
    pub result: Result<Decoded, DecodeError>,
}
/// Fixed concurrency and queue capacity bound both transient and waiting pixels.
pub struct DecodePool {
    /// A bounded queue prevents callers accumulating work.
    pub requests: Requests,
    /// Backpressure caps memory while the UI is busy.
    pub results: Receiver<Completion>,
    cache: Arc<LodCache>,
    baker: Arc<bake::Baker>,
}
/// Reads a whole original within the input budget.
fn read_original(storage: &dyn Storage, handle: Handle) -> Result<Vec<u8>, DecodeError> {
    let mut reader = storage.open(handle)?;
    let len = reader.seek(std::io::SeekFrom::End(0))?;
    if len > MAX_INPUT_BYTES {
        return Err(DecodeError::Budget);
    }
    reader.seek(std::io::SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(len as usize);
    reader.take(MAX_INPUT_BYTES + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}
fn decode_request(
    storage: &dyn Storage,
    baker: &bake::Baker,
    cache: &LodCache,
    request: Request,
    batch: &[Request],
    decoded_tiles: &mut Vec<Decoded>,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Completion {
    let mut failure_hash = None;
    let result = (|| {
        if cancelled.load(Ordering::Relaxed) {
            return Err(DecodeError::Cancelled);
        }
        // A baked pack answers tiles without the original or this process's RAM.
        if let Some(hash) = request.blob
            && let Some(tile) = request.tile
            && let Some(image) = cache.pack_tile(hash, request.format, tile)
        {
            return Ok(image);
        }
        if let Some(hash) = request.blob
            && let Some(mut image) = cache.lookup(
                request.tile.map_or(hash, |tile| tile.key(hash)),
                request.format,
                request.side,
            )
        {
            image.hash = hash;
            image.tile = request.tile;
            baker.offer(cache, request.handle, hash, request.format, image.original);
            return Ok(image);
        }
        let image = if let Some(index) = decoded_tiles
            .iter()
            .position(|image| image.tile == request.tile)
        {
            decoded_tiles.swap_remove(index)
        } else {
            let bytes = read_original(storage, request.handle)?;
            if cancelled.load(Ordering::Relaxed) {
                return Err(DecodeError::Cancelled);
            }
            let result = if request.tile.is_some() {
                let tiles: Vec<_> = batch.iter().filter_map(|r| r.tile).collect();
                tiles::decode_batch(&bytes, &tiles, cancelled).map(|images| {
                    *decoded_tiles = images;
                    let index = decoded_tiles
                        .iter()
                        .position(|image| image.tile == request.tile)
                        .expect("batch decoder preserves every requested tile");
                    decoded_tiles.swap_remove(index)
                })
            } else {
                // Coarse fallbacks must stay coarse; tiles supply later detail.
                let side = if request.side <= tiles::SIDE {
                    request.side
                } else {
                    cache.decode_side(request.side, request.blob.is_some())
                };
                // shortcut: whole-preview codecs finish the current call; add row tokens if cancellation latency becomes visible.
                decode(&bytes, side)
            };
            if result.is_err() {
                failure_hash = Some(blake3::hash(&bytes));
            }
            result?
        };
        if cancelled.load(Ordering::Relaxed) {
            return Err(DecodeError::Cancelled);
        }
        let mut image = apply_format(request.format, image);
        if cancelled.load(Ordering::Relaxed) {
            return Err(DecodeError::Cancelled);
        }
        let hash = image.hash;
        if let Some(tile) = image.tile {
            image.hash = tile.key(hash);
            image.tile = None;
        }
        if request.blob.is_some() {
            // First sight of a large image: bake its tiles before the user zooms in.
            baker.offer(cache, request.handle, hash, request.format, image.original);
        }
        let mut image = cache.keep(image, request.side);
        image.hash = hash;
        image.tile = request.tile;
        Ok(image)
    })();
    Completion {
        request,
        result,
        failure_hash,
    }
}
impl DecodePool {
    /// Wakes the event loop only when new work is ready. Uses a private cache that
    /// stays in [`CacheMode::NoCache`] unless configured.
    pub fn new(
        storage: Arc<dyn Storage>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, std::io::Error> {
        Self::with_cache(storage, wake, Arc::new(LodCache::new()))
    }
    /// Like [`DecodePool::new`], with a cache shared by other pools (board tabs).
    pub fn with_cache(
        storage: Arc<dyn Storage>,
        wake: Arc<dyn Fn() + Send + Sync>,
        cache: Arc<LodCache>,
    ) -> Result<Self, std::io::Error> {
        let threads = std::thread::available_parallelism()?
            .get()
            .saturating_sub(1)
            .max(1);
        let workers = threads.min(4);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|n| format!("decode-{n}"))
            .build()
            .map_err(std::io::Error::other)?;
        let requests = Requests::default();
        let workers_rx: Vec<_> = (0..workers).map(|_| requests.worker()).collect();
        // One complete tile batch can become ready between UI frames.
        let (tx, results) = crossbeam_channel::bounded(8);
        let shared = cache.clone();
        let baker = Arc::new(bake::Baker::new(storage.clone(), cache.clone())?);
        let bakers = baker.clone();
        std::thread::Builder::new()
            .name("decode-dispatch".into())
            .spawn(move || {
                pool.scope(|scope| {
                    for rx in workers_rx {
                        let tx = tx.clone();
                        let storage = storage.clone();
                        let wake = wake.clone();
                        let cache = shared.clone();
                        let baker = bakers.clone();
                        scope.spawn(move |_| {
                            while let Some((batch, cancelled)) = rx.next_batch() {
                                let mut decoded_tiles = Vec::new();
                                for request in batch.iter().copied() {
                                    let completion = decode_request(
                                        storage.as_ref(),
                                        &baker,
                                        &cache,
                                        request,
                                        &batch,
                                        &mut decoded_tiles,
                                        &cancelled,
                                    );
                                    if !rx.finish(request, &cancelled) {
                                        continue;
                                    }
                                    if tx.send(completion).is_err() {
                                        return;
                                    }
                                    wake();
                                }
                            }
                        });
                    }
                });
            })?;
        Ok(Self {
            requests,
            results,
            cache,
            baker,
        })
    }
    /// The cache this pool reads and fills.
    pub fn cache(&self) -> &Arc<LodCache> {
        &self.cache
    }
    /// Queues every image of the board for a background tile bake, behind images on
    /// screen. Only [`BakeMode::Board`] does anything; repeated calls queue nothing new.
    pub fn offer_board(
        &self,
        format: ImageFormat,
        images: impl IntoIterator<Item = (Handle, blake3::Hash)>,
    ) {
        self.baker.offer_board(&self.cache, format, images);
    }
    /// Images waiting for or undergoing a background tile bake.
    pub fn bake_pending(&self) -> usize {
        self.baker.pending()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coarse_fallback_does_not_cache_a_megapixel_preview() {
        struct Source;
        impl Storage for Source {
            fn open(&self, _: Handle) -> std::io::Result<Box<dyn moth_format::ReadSeek>> {
                Ok(Box::new(std::io::Cursor::new(
                    include_bytes!("../../../corpus/images/4k.jpg").as_slice(),
                )))
            }
            fn append(&self, _: Handle) -> std::io::Result<Box<dyn Write + Send>> {
                Err(std::io::ErrorKind::Unsupported.into())
            }
            fn sync(&self, _: Handle) -> std::io::Result<()> {
                Ok(())
            }
        }
        let cache = Arc::new(LodCache::new());
        cache.set_mode(CacheMode::Memory);
        cache.set_memory_budget(16 * 1024 * 1024);
        let pool =
            DecodePool::with_cache(Arc::new(Source), Arc::new(|| {}), cache.clone()).unwrap();
        pool.requests.schedule([Request {
            handle: Handle(0),
            blob: Some(blake3::hash(include_bytes!(
                "../../../corpus/images/4k.jpg"
            ))),
            tile: None,
            side: tiles::SIDE,
            format: ImageFormat::Rgba8,
        }]);
        pool.results
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .result
            .unwrap();
        assert!(
            cache.memory_bytes() <= 256 * 1024,
            "coarse fallback retained {} bytes",
            cache.memory_bytes()
        );
    }
    #[test]
    fn neighboring_tiles_share_one_source_read() {
        struct Source(Arc<std::sync::atomic::AtomicUsize>);
        impl Storage for Source {
            fn open(&self, _: Handle) -> std::io::Result<Box<dyn moth_format::ReadSeek>> {
                self.0.fetch_add(1, Ordering::Relaxed);
                Ok(Box::new(std::io::Cursor::new(
                    include_bytes!("../../../corpus/images/4k.jpg").as_slice(),
                )))
            }
            fn append(&self, _: Handle) -> std::io::Result<Box<dyn Write + Send>> {
                Err(std::io::ErrorKind::Unsupported.into())
            }
            fn sync(&self, _: Handle) -> std::io::Result<()> {
                Ok(())
            }
        }
        let opens = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (ready, notifications) = crossbeam_channel::bounded(8);
        let pool = DecodePool::new(
            Arc::new(Source(opens.clone())),
            Arc::new(move || {
                let _ = ready.try_send(());
            }),
        )
        .unwrap();
        pool.requests.schedule((0..8).map(|x| Request {
            handle: Handle(0),
            blob: Some(blake3::hash(include_bytes!(
                "../../../corpus/images/4k.jpg"
            ))),
            tile: Some(tiles::Tile { level: 1, x, y: 1 }),
            side: tiles::SIDE,
            format: ImageFormat::Rgba8,
        }));
        // The UI may not drain results until the next frame. A complete batch must fit.
        for _ in 0..8 {
            notifications
                .recv_timeout(std::time::Duration::from_secs(2))
                .expect("a whole tile batch must be ready before the UI drains results");
        }
        for _ in 0..8 {
            let result = pool
                .results
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            assert_eq!(result.result.unwrap().tile, result.request.tile);
        }
        assert_eq!(
            opens.load(Ordering::Relaxed),
            1,
            "neighboring tiles repeatedly read and decode the same original"
        );
    }
    #[test]
    fn specialized_rows_match_scalar_area_sums() {
        for color in [
            png::ColorType::Grayscale,
            png::ColorType::GrayscaleAlpha,
            png::ColorType::Rgb,
            png::ColorType::Rgba,
        ] {
            for side in [1, 8, 31] {
                for (start, step) in [(0, 1), (0, 8), (4, 8), (2, 4), (1, 2)] {
                    let mut preview = AreaPreview::new(31, 1, side, 1);
                    let bytes: Vec<_> = (0..(31u32 - start).div_ceil(step) as usize
                        * color.samples())
                        .map(|n| (n * 53) as u8)
                        .collect();
                    preview.row(0, start, step, &bytes, color).unwrap();
                    let mut expected = vec![[0u64; 4]; side as usize];
                    for (i, p) in bytes.chunks_exact(color.samples()).enumerate() {
                        let rgba = match color {
                            png::ColorType::Grayscale => [p[0], p[0], p[0], 255],
                            png::ColorType::GrayscaleAlpha => [p[0], p[0], p[0], p[1]],
                            png::ColorType::Rgb => [p[0], p[1], p[2], 255],
                            _ => [p[0], p[1], p[2], p[3]],
                        };
                        let sum = &mut expected[((start + i as u32 * step) * side / 31) as usize];
                        for c in 0..3 {
                            sum[c] += u64::from(rgba[c]) * u64::from(rgba[3]);
                        }
                        sum[3] += u64::from(rgba[3]);
                    }
                    let actual = preview.finish();
                    for (x, (pixel, sum)) in actual.pixels().zip(expected).enumerate() {
                        let count = (((x + 1) as u64 * 31).div_ceil(u64::from(side))
                            - (x as u64 * 31).div_ceil(u64::from(side)))
                        .max(1);
                        let expected = [
                            (sum[0] + sum[3] / 2).checked_div(sum[3]).unwrap_or(0) as u8,
                            (sum[1] + sum[3] / 2).checked_div(sum[3]).unwrap_or(0) as u8,
                            (sum[2] + sum[3] / 2).checked_div(sum[3]).unwrap_or(0) as u8,
                            ((sum[3] + count / 2) / count) as u8,
                        ];
                        assert_eq!(pixel.0, expected);
                    }
                }
            }
        }
    }
    #[test]
    fn webp_orientation_and_inner_dimension_budget() {
        let bytes = include_bytes!("../../../corpus/images/oriented.webp");
        let mut reference = image::load_from_memory(bytes).unwrap();
        reference.apply_orientation(image::metadata::Orientation::Rotate90);
        let result = decode(bytes, 64).unwrap();
        assert_eq!(result.original, [17, 31]);
        assert_eq!(result.mips[0].bytes, reference.into_rgba8().into_raw());
        for bytes in [
            include_bytes!("../../../corpus/images/lossless.webp").as_slice(),
            include_bytes!("../../../corpus/images/lossy.webp").as_slice(),
        ] {
            assert!(preview::webp_dimensions(bytes, 1, 1).is_err());
            assert!(preview::webp_dimensions(&bytes[..bytes.len() - 2], 31, 17).is_err());
        }
    }
    #[test]
    fn bounded_webp_matches_reference_pixels() {
        for bytes in [
            include_bytes!("../../../corpus/images/lossless.webp").as_slice(),
            include_bytes!("../../../corpus/images/lossy.webp").as_slice(),
            include_bytes!("../../../corpus/images/alpha.webp").as_slice(),
        ] {
            let source = image::load_from_memory(bytes).unwrap().into_rgba8();
            for side in [1, 8, 64] {
                let expected = preview_rgba(31, 17, source.as_raw(), side).unwrap();
                let actual = decode(bytes, side).unwrap();
                assert_eq!(actual.original, [31, 17]);
                assert_eq!(actual.mips[0].bytes, expected.mips[0].bytes);
            }
        }
        assert!(matches!(
            decode(include_bytes!("../tests/fixtures/animated.webp"), 64),
            Err(DecodeError::Budget)
        ));
        let mut oversized_input = include_bytes!("../../../corpus/images/lossless.webp").to_vec();
        oversized_input.resize(8 * 1024 * 1024 + 1, 0);
        assert!(matches!(
            decode(&oversized_input, 64),
            Err(DecodeError::Budget)
        ));
        let mut oversized = include_bytes!("../../../corpus/images/budget.webp").to_vec();
        assert_eq!(&oversized[12..16], b"VP8L");
        let dimensions = (3840u32 - 1) | ((2160u32 - 1) << 14);
        oversized[21..25].copy_from_slice(&dimensions.to_le_bytes());
        assert!(matches!(decode(&oversized, 64), Err(DecodeError::Budget)));
    }
    #[test]
    fn gif_offsets_and_transparency_match_reference() {
        let bytes = include_bytes!("../../../corpus/images/offset.gif");
        let mut source = image::load_from_memory(bytes).unwrap().into_rgba8();
        for pixel in source.pixels_mut().filter(|p| p[3] == 0) {
            *pixel = image::Rgba([0; 4]);
        }
        for side in [1, 4, 16] {
            let expected = preview_rgba(7, 5, source.as_raw(), side).unwrap();
            let actual = decode(bytes, side).unwrap();
            assert_eq!(actual.original, [7, 5]);
            assert_eq!(actual.mips[0].bytes, expected.mips[0].bytes);
        }
    }
    #[test]
    fn bmp_top_down_and_invalid_layouts() {
        let bytes = include_bytes!("../../../corpus/images/rows.bmp");
        let expected = decode(bytes, 8).unwrap();
        let mut top_down = bytes.to_vec();
        let offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
        let stride = (31usize * 24).div_ceil(32) * 4;
        for y in 0..17 {
            top_down[offset + y * stride..offset + (y + 1) * stride]
                .copy_from_slice(&bytes[offset + (16 - y) * stride..offset + (17 - y) * stride]);
        }
        top_down[22..26].copy_from_slice(&(-17i32).to_le_bytes());
        assert_eq!(
            decode(&top_down, 8).unwrap().mips[0].bytes,
            expected.mips[0].bytes
        );
        for (offset, value) in [(10, 0u32), (10, u32::MAX), (30, 1), (30, 2)] {
            let mut invalid = bytes.to_vec();
            invalid[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&invalid, 8).is_err());
        }
        for end in [0, 13, 53, bytes.len() - 1] {
            assert!(decode(&bytes[..end], 8).is_err());
        }
    }
    #[test]
    fn gif_and_bmp_previews_are_area_averaged() {
        for bytes in [
            include_bytes!("../../../corpus/images/rows.gif").as_slice(),
            include_bytes!("../../../corpus/images/interlaced.gif").as_slice(),
            include_bytes!("../../../corpus/images/rows.bmp").as_slice(),
            include_bytes!("../../../corpus/images/palette.bmp").as_slice(),
            include_bytes!("../../../corpus/images/alpha.bmp").as_slice(),
        ] {
            let source = image::load_from_memory(bytes).unwrap().into_rgba8();
            let expected = preview_rgba(31, 17, source.as_raw(), 8).unwrap();
            let result = decode(bytes, 8).unwrap();
            assert_eq!(result.original, [31, 17]);
            assert_eq!(result.mips[0].bytes, expected.mips[0].bytes);
            assert_eq!(decode(bytes, 64).unwrap().mips[0].bytes, source.into_raw());
        }
    }
    #[test]
    fn streamed_hash_tracks_only_successful_writes() {
        struct ShortWriter(Vec<u8>);
        impl Write for ShortWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                let len = bytes.len().min(3);
                self.0.extend_from_slice(&bytes[..len]);
                Ok(len)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = HashWriter {
            writer: ShortWriter(Vec::new()),
            hash: blake3::Hasher::new(),
        };
        writer.write_all(b"clipboard original").unwrap();
        assert_eq!(writer.hash.finalize(), blake3::hash(&writer.writer.0));
        let hash = blake3::hash(b"encoded original");
        let pixels = [10, 20, 30, 255].repeat(16);
        let preview = preview_rgba_with_hash(4, 4, &pixels, 2, hash).unwrap();
        assert_eq!(preview.hash, hash);
        assert_eq!(preview.mips[0].bytes, [10, 20, 30, 255].repeat(4));
        assert!(preview_rgba_with_hash(4, 4, &pixels[..4], 2, hash).is_err());
    }
    #[test]
    fn rgba_scratch_round_trip_and_torn_write() {
        let pixels = [20, 40, 60, 255].repeat(16 * 8);
        let mut bytes = Vec::new();
        write_rgba(&mut bytes, 16, 8, &pixels).unwrap();
        let image = decode(&bytes, 16).unwrap();
        assert_eq!(image.original, [16, 8]);
        let direct = preview_rgba(16, 8, &pixels, 16).unwrap();
        assert_eq!(direct.hash, image.hash);
        assert_eq!(direct.mips[0].bytes, image.mips[0].bytes);
        assert_eq!(image.mips[0].bytes, pixels);
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end], 16).is_err());
        }
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(decode(&bytes, 16).is_err());
    }
    #[test]
    fn rejects_corrupt_input_without_panicking() {
        assert!(decode(b"garbage", 64).is_err());
    }
    #[test]
    fn mip_averaging_preserves_base_pixels_and_thin_image_edges() {
        for (width, height) in [(32, 16), (31, 17), (1, 17), (17, 1)] {
            let source =
                image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
            let decoded = pyramid(source.clone(), blake3::hash(b"constant"), [width, height]);
            assert_eq!(&decoded.mips[0].bytes, source.as_raw());
            assert!(decoded.mips.iter().all(|m| {
                m.bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| *p == [10, 20, 30, 255])
            }));
            assert_eq!(
                decoded.mips.last().map(|m| [m.width, m.height]),
                Some([1, 1])
            );
        }
        let source = image::RgbaImage::from_fn(32, 16, |x, y| {
            image::Rgba([if (x + y) % 2 == 0 { 0 } else { 255 }; 4])
        });
        let decoded = pyramid(source.clone(), blake3::hash(b"checker"), [32, 16]);
        assert_eq!(&decoded.mips[0].bytes, source.as_raw());
        assert!(
            decoded
                .mips
                .iter()
                .skip(1)
                .all(|m| m.bytes.iter().all(|v| v.abs_diff(128) <= 1))
        );
    }
    #[test]
    fn pyramid_reaches_one_pixel() {
        let image = image::RgbaImage::from_pixel(31, 17, image::Rgba([10, 20, 30, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let decoded = decode(bytes.get_ref(), 16).unwrap();
        assert_eq!(decoded.original, [31, 17]);
        assert_eq!(decoded.mips[0].width, 16);
        assert_eq!(
            decoded.mips.last().map(|m| (m.width, m.height)),
            Some((1, 1))
        );
    }
    #[test]
    fn png_rows_and_adam7_match_area_average() {
        let mut previous = None;
        for bytes in [
            include_bytes!("../../../corpus/images/rows.png").as_slice(),
            include_bytes!("../../../corpus/images/adam7.png").as_slice(),
        ] {
            let source = image::load_from_memory(bytes).unwrap().into_rgba8();
            let preview = decode(bytes, 16).unwrap();
            let direct =
                preview_rgba(source.width(), source.height(), source.as_raw(), 16).unwrap();
            assert_eq!(preview.mips[0].bytes, direct.mips[0].bytes);
            if let Some(previous) = previous {
                assert_eq!(preview.mips[0].bytes, previous);
            }
            previous = Some(preview.mips[0].bytes.clone());
        }
    }
    #[test]
    fn large_jpeg_uses_reduced_budget() {
        let decoded = decode(include_bytes!("../../../corpus/images/large-dct.jpg"), 256).unwrap();
        assert_eq!(decoded.original, [8192, 4097]);
        assert_eq!(decoded.mips[0].width, 256);
        let mut progressive = include_bytes!("../../../corpus/images/large-dct.jpg").to_vec();
        let marker = progressive
            .windows(2)
            .position(|marker| marker == [0xff, 0xc0])
            .unwrap();
        progressive[marker + 1] = 0xc2;
        assert!(matches!(
            decode(&progressive, 256),
            Err(DecodeError::Budget)
        ));
    }
    #[test]
    fn jpeg_orientation_and_cmyk() {
        let decoded = decode(include_bytes!("../../../corpus/images/oriented.jpg"), 16).unwrap();
        assert_eq!(decoded.original, [4, 8]);
        assert_eq!((decoded.mips[0].width, decoded.mips[0].height), (4, 8));
        let decoded = decode(include_bytes!("../../../corpus/images/cmyk.jpg"), 16).unwrap();
        assert_eq!(decoded.original, [8, 4]);
        for pixel in decoded.mips[0].bytes.as_chunks::<4>().0 {
            for (actual, expected) in pixel.iter().zip([255u8, 127, 0, 255]) {
                assert!(actual.abs_diff(expected) <= 2);
            }
        }
    }
    #[test]
    fn png_omits_only_redundant_alpha() {
        for alpha in [255, 254, 0] {
            let mut pixels = Vec::new();
            for index in 0..31 * 17 {
                pixels.extend_from_slice(&[index as u8, (index / 31) as u8, 60, 255]);
            }
            let last = pixels.len() - 1;
            pixels[last] = alpha;
            let mut encoded = Vec::new();
            let hash = write_png(&mut encoded, 31, 17, &pixels).unwrap();
            assert_eq!(hash, blake3::hash(&encoded));
            let reader = png::Decoder::new(std::io::Cursor::new(&encoded))
                .read_info()
                .unwrap();
            assert_eq!(
                reader.info().color_type,
                if alpha == 255 {
                    png::ColorType::Rgb
                } else {
                    png::ColorType::Rgba
                }
            );
            assert_eq!(
                image::load_from_memory(&encoded)
                    .unwrap()
                    .into_rgba8()
                    .as_raw(),
                &pixels
            );
        }
    }
    #[test]
    fn previews_average_all_source_pixels() {
        let pixels = [0, 0, 0, 255, 255, 255, 255, 255].repeat(2);
        let direct = preview_rgba(2, 2, &pixels, 1).unwrap();
        assert_eq!(direct.mips[0].bytes, [128, 128, 128, 255]);
        let mut encoded = Vec::new();
        write_png(&mut encoded, 2, 2, &pixels).unwrap();
        assert_eq!(
            decode(&encoded, 1).unwrap().mips[0].bytes,
            direct.mips[0].bytes
        );
    }
    #[test]
    fn transparent_colors_do_not_bleed_into_area_previews() {
        let pixels = [255, 0, 0, 0, 0, 0, 255, 255];
        let preview = preview_rgba(2, 1, &pixels, 1).unwrap();
        assert_eq!(preview.mips[0].bytes, [0, 0, 255, 128]);
        let mut encoded = Vec::new();
        write_png(&mut encoded, 2, 1, &pixels).unwrap();
        assert_eq!(
            decode(&encoded, 1).unwrap().mips[0].bytes,
            preview.mips[0].bytes
        );
    }
    #[test]
    fn oversized_header_rejected() {
        let image = image::RgbaImage::new(1, 1);
        let mut bytes = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let mut bytes = bytes.into_inner();
        bytes[16..20].copy_from_slice(&100000u32.to_be_bytes());
        assert!(decode(&bytes, 64).is_err());
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn bc7_round_trip_stays_within_tolerance() {
        let color = [128u8, 64, 32, 255];
        // Block-aligned surface round-trips closely (BC7 is lossy).
        let encoded = encode_bc7(&color.repeat(8 * 8), 8, 8);
        assert_eq!(encoded.len(), 64); // 2x2 blocks of 16 bytes
        let mut decoded = vec![0u8; 8 * 8 * 4];
        for (block, out) in encoded
            .as_chunks::<16>()
            .0
            .iter()
            .zip(decoded.as_chunks_mut::<64>().0.iter_mut())
        {
            bcdec_rs::bc7(block, out, 16);
        }
        let worst = decoded
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| {
                p[..3]
                    .iter()
                    .zip(color[..3].iter())
                    .map(|(&a, &b)| a.abs_diff(b))
            })
            .max()
            .unwrap();
        assert!(worst <= 4, "BC7 solid-color error {worst} exceeds 4");
        // Non-block-aligned mips are padded to the 4x4 grid, not truncated.
        assert_eq!(encode_bc7(&color.repeat(5 * 3), 5, 3).len(), 32);
    }
}
