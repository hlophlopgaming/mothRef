// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Bounded image workers. Pixels never enter the scene model.
use crossbeam_channel::{Receiver, Sender};
use image::{DynamicImage, ImageReader};
use moth_format::{Handle, Storage};
use std::{
    io::{Read, Write},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

/// Prevents compressed input from exhausting memory before header validation.
pub const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
/// Bounds transient decoding allocations, including malicious image headers.
pub const MAX_PIXELS: u64 = 32 * 1024 * 1024;
/// Largest preview; full-resolution visible tiles are a separate future path.
pub const MAX_SIDE: u32 = 2048;
/// A single rebuildable LOD, suitable for one array layer.
pub struct Mip {
    /// Positive extents preserve valid geometry and aspect ratio.
    pub width: u32,
    /// Positive extents preserve valid geometry and aspect ratio.
    pub height: u32,
    /// Owned pixels can cross worker boundaries without borrowing a decoder.
    pub rgba: Vec<u8>,
}
/// Original identity survives changes in preview resolution or GPU encoding.
pub struct Decoded {
    /// Deduplicates original content independently of preview resolution.
    pub hash: blake3::Hash,
    /// Preserves aspect ratio without retaining full-resolution pixels.
    pub original: [u32; 2],
    /// Lower levels let the renderer avoid oversized image samples.
    pub mips: Vec<Mip>,
}
/// Identifies a requested preview without giving core code a native path.
#[derive(Clone, Copy, Debug)]
pub struct Request {
    /// Opaque identity keeps native paths out of core crates.
    pub handle: Handle,
    /// Bounds preview work independently of source dimensions.
    pub side: u32,
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
    if w == 0 || h == 0 || w > 32768 || h > 32768 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(DecodeError::Budget);
    }
    let side = side.clamp(1, MAX_SIDE);
    let image = if format == Some(image::ImageFormat::Jpeg) {
        let mut jpeg = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
        let divisor = [1u32, 2, 4, 8]
            .into_iter()
            .find(|d| w.max(h).div_ceil(*d) <= side)
            .unwrap_or(8);
        jpeg.scale(w.div_ceil(divisor) as u16, h.div_ceil(divisor) as u16)?;
        let pixels = jpeg.decode()?;
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
            _ => return Err(DecodeError::Budget),
        }
    } else if format == Some(image::ImageFormat::Png) {
        DynamicImage::ImageRgba8(png_preview(bytes, w, h, side)?)
    } else {
        let mut reader = ImageReader::with_format(
            std::io::Cursor::new(bytes),
            format.ok_or(DecodeError::Budget)?,
        );
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(MAX_PIXELS * 8);
        limits.max_image_width = Some(32768);
        limits.max_image_height = Some(32768);
        reader.limits(limits);
        reader.decode()?
    };
    Ok(pyramid(
        image.thumbnail(side, side).into_rgba8(),
        blake3::hash(bytes),
        [w, h],
    ))
}
fn pyramid(mut level: image::RgbaImage, hash: blake3::Hash, original: [u32; 2]) -> Decoded {
    let mut mips = Vec::new();
    loop {
        let (width, height) = level.dimensions();
        let next = if width > 1 || height > 1 {
            Some(image::imageops::resize(
                &level,
                (width / 2).max(1),
                (height / 2).max(1),
                image::imageops::FilterType::Triangle,
            ))
        } else {
            None
        };
        mips.push(Mip {
            width,
            height,
            rgba: level.into_raw(),
        });
        match next {
            Some(n) => level = n,
            None => break,
        }
    }
    Decoded {
        hash,
        original,
        mips,
    }
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
/// Compresses clipboard originals losslessly into a portable, checksummed image.
pub fn write_png(
    writer: impl Write,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), DecodeError> {
    validate_rgba(width, height, rgba.len())?;
    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header()?;
    {
        let mut stream = writer.stream_writer()?;
        stream.write_all(rgba)?;
        stream.finish()?;
    }
    writer.finish()?;
    Ok(())
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
    let mut image = image::RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let at = (((y * height / h) * width + x * width / w) * 4) as usize;
            let p = &pixels[at..at + 4];
            image.put_pixel(x, y, image::Rgba([p[0], p[1], p[2], p[3]]));
        }
    }
    Ok(pyramid(image, hash, [width, height]))
}
// Stream PNG rows into the requested LOD, including Adam7 passes; never allocate the source frame.
fn png_preview(
    bytes: &[u8],
    width: u32,
    height: u32,
    side: u32,
) -> Result<image::RgbaImage, DecodeError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: 8 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let ratio = (side as f64 / f64::from(width.max(height))).min(1.);
    let w = (f64::from(width) * ratio).round().max(1.) as u32;
    let h = (f64::from(height) * ratio).round().max(1.) as u32;
    let mut output = image::RgbaImage::new(w, h);
    let (color, depth) = reader.output_color_type();
    if depth != png::BitDepth::Eight {
        return Err(DecodeError::Budget);
    }
    let channels = color.samples();
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
            let dy = (u64::from(y) * u64::from(h)).div_ceil(u64::from(height)) as u32;
            if dy >= h || dy * height / h != y {
                continue;
            }
            for dx in 0..w {
                let x = dx * width / w;
                if x < x_start || !(x - x_start).is_multiple_of(x_step) {
                    continue;
                }
                let at = ((x - x_start) / x_step) as usize * channels;
                let p = row
                    .data()
                    .get(at..at + channels)
                    .ok_or(DecodeError::Budget)?;
                let rgba = match color {
                    png::ColorType::Grayscale => [p[0], p[0], p[0], 255],
                    png::ColorType::GrayscaleAlpha => [p[0], p[0], p[0], p[1]],
                    png::ColorType::Rgb => [p[0], p[1], p[2], 255],
                    png::ColorType::Rgba => [p[0], p[1], p[2], p[3]],
                    png::ColorType::Indexed => return Err(DecodeError::Budget),
                };
                output.put_pixel(dx, dy, image::Rgba(rgba));
            }
        }
    }
    reader.finish()?;
    Ok(output)
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
    pub requests: Sender<Request>,
    /// Backpressure caps memory while the UI is busy.
    pub results: Receiver<Completion>,
}
impl DecodePool {
    /// Wakes the event loop only when new work is ready.
    pub fn new(
        storage: Arc<dyn Storage>,
        wake: Arc<dyn Fn() + Send + Sync>,
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
        let (requests, rx) = crossbeam_channel::bounded::<Request>(8);
        let (tx, results) = crossbeam_channel::bounded(2);
        std::thread::Builder::new()
            .name("decode-dispatch".into())
            .spawn(move || {
                pool.scope(|scope| {
                    for _ in 0..workers {
                        let rx = rx.clone();
                        let tx = tx.clone();
                        let storage = storage.clone();
                        let wake = wake.clone();
                        scope.spawn(move |_| {
                            while let Ok(request) = rx.recv() {
                                let mut failure_hash = None;
                                let result = (|| {
                                    let mut reader = storage.open(request.handle)?;
                                    let len = reader.seek(std::io::SeekFrom::End(0))?;
                                    if len > MAX_INPUT_BYTES {
                                        return Err(DecodeError::Budget);
                                    }
                                    reader.seek(std::io::SeekFrom::Start(0))?;
                                    let mut bytes = Vec::with_capacity(len as usize);
                                    reader.take(MAX_INPUT_BYTES + 1).read_to_end(&mut bytes)?;
                                    let result = decode(&bytes, request.side);
                                    if result.is_err() {
                                        failure_hash = Some(blake3::hash(&bytes));
                                    }
                                    result
                                })();
                                if tx
                                    .send(Completion {
                                        request,
                                        result,
                                        failure_hash,
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                                wake();
                            }
                        });
                    }
                });
            })?;
        Ok(Self { requests, results })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgba_scratch_round_trip_and_torn_write() {
        let pixels = [20, 40, 60, 255].repeat(16 * 8);
        let mut bytes = Vec::new();
        write_rgba(&mut bytes, 16, 8, &pixels).unwrap();
        let image = decode(&bytes, 16).unwrap();
        assert_eq!(image.original, [16, 8]);
        let direct = preview_rgba(16, 8, &pixels, 16).unwrap();
        assert_eq!(direct.hash, image.hash);
        assert_eq!(direct.mips[0].rgba, image.mips[0].rgba);
        assert_eq!(image.mips[0].rgba, pixels);
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
    fn png_rows_and_adam7_match_source_samples() {
        for bytes in [
            include_bytes!("../../../corpus/images/rows.png").as_slice(),
            include_bytes!("../../../corpus/images/adam7.png").as_slice(),
        ] {
            let source = image::load_from_memory(bytes).unwrap().into_rgba8();
            let preview = decode(bytes, 16).unwrap();
            let mip = &preview.mips[0];
            for y in 0..mip.height {
                for x in 0..mip.width {
                    let expected = source.get_pixel(
                        x * source.width() / mip.width,
                        y * source.height() / mip.height,
                    );
                    assert_eq!(
                        &mip.rgba[((y * mip.width + x) * 4) as usize
                            ..((y * mip.width + x) * 4 + 4) as usize],
                        &expected.0
                    );
                }
            }
        }
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
}
