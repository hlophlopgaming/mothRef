// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded timing inspection and sequential compositing for animated GIFs.
use crate::{DecodeError, Decoded, ImageFormat};
use image::{AnimationDecoder, Frames, ImageDecoder, RgbaImage};
use moth_format::{Handle, Storage};
use std::{
    io::{BufReader, Read, Seek, SeekFrom},
    panic::{AssertUnwindSafe, catch_unwind},
    time::Duration,
};

const MAX_FRAMES: usize = 10_000;
const MAX_PIXELS: u64 = 16 * 1024 * 1024;
const PREVIEW_SIDE: u32 = 512;

fn source(
    storage: &dyn Storage,
    handle: Handle,
) -> Result<Box<dyn moth_format::ReadSeek>, DecodeError> {
    let mut input = storage.open(handle)?;
    if input.seek(SeekFrom::End(0))? > crate::MAX_INPUT_BYTES {
        return Err(DecodeError::Budget);
    }
    input.seek(SeekFrom::Start(0))?;
    Ok(input)
}

fn dimensions(w: u32, h: u32) -> Result<u64, DecodeError> {
    let pixels = u64::from(w) * u64::from(h);
    if pixels == 0 || pixels > MAX_PIXELS || w > crate::MAX_DIMENSION || h > crate::MAX_DIMENSION {
        return Err(DecodeError::Budget);
    }
    Ok(pixels)
}

fn metadata(input: impl Read) -> Result<gif::Decoder<impl Read>, DecodeError> {
    let mut options = gif::DecodeOptions::new();
    options.skip_frame_decoding(true);
    options.check_frame_consistency(true);
    Ok(options.read_info(BufReader::new(input))?)
}

pub(crate) fn has_animation(bytes: &[u8]) -> Result<bool, DecodeError> {
    let mut reader = metadata(bytes)?;
    Ok(reader.next_frame_info()?.is_some() && reader.next_frame_info()?.is_some())
}

/// Inspects frame delays without allocating decoded pixels. Non-GIF and one-frame images stay static.
pub fn timeline(
    storage: &dyn Storage,
    handle: Handle,
) -> Result<Option<Vec<Duration>>, DecodeError> {
    catch_unwind(AssertUnwindSafe(|| {
        let mut input = storage.open(handle)?;
        let mut header = [0; 6];
        if input.read_exact(&mut header).is_err() || !matches!(&header, b"GIF87a" | b"GIF89a") {
            return Ok(None);
        }
        if input.seek(SeekFrom::End(0))? > crate::MAX_INPUT_BYTES {
            return Err(DecodeError::Budget);
        }
        input.seek(SeekFrom::Start(0))?;
        let mut reader = metadata(input)?;
        let mut delays = Vec::new();
        while let Some(frame) = reader.next_frame_info()? {
            if delays.len() == MAX_FRAMES || frame.width == 0 || frame.height == 0 {
                return Err(DecodeError::Budget);
            }
            // Common GIF viewers treat absent or single-tick delays as 100 ms.
            let delay = if frame.delay <= 1 {
                100
            } else {
                u64::from(frame.delay) * 10
            };
            delays.push(Duration::from_millis(delay));
        }
        if delays.len() < 2 {
            return Ok(None);
        }
        dimensions(reader.width().into(), reader.height().into())?;
        Ok(Some(delays))
    }))
    .map_err(|_| DecodeError::Panic)?
}

/// Keeps only the compositor's current canvas. Seeking backwards reopens the original.
#[derive(Default)]
pub struct Reader {
    frames: Option<Frames<'static>>,
    admission: Option<crate::frames::Admission>,
    next: usize,
}

impl Reader {
    /// Reads the full composited frame, including transparency and the earlier frames' disposal.
    pub fn frame(
        &mut self,
        storage: &dyn Storage,
        handle: Handle,
        index: usize,
    ) -> Result<RgbaImage, DecodeError> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if index >= MAX_FRAMES {
                return Err(DecodeError::Budget);
            }
            if self.frames.is_none() || index < self.next {
                *self = Self::default();
                let mut decoder =
                    image::codecs::gif::GifDecoder::new(BufReader::new(source(storage, handle)?))?;
                let (w, h) = decoder.dimensions();
                let pixels = dimensions(w, h)?;
                let mut limits = image::Limits::default();
                limits.max_alloc = Some(pixels * 12);
                self.admission = crate::frames::admit(pixels * 12);
                limits.max_image_width = Some(crate::MAX_DIMENSION);
                limits.max_image_height = Some(crate::MAX_DIMENSION);
                decoder.set_limits(limits)?;
                self.frames = Some(decoder.into_frames());
                self.next = 0;
            }
            let frames = self.frames.as_mut().ok_or(DecodeError::Budget)?;
            while let Some(frame) = frames.next().transpose()? {
                let current = self.next;
                self.next += 1;
                if current == index {
                    return Ok(frame.into_buffer());
                }
            }
            Err(DecodeError::Budget)
        }))
        .map_err(|_| DecodeError::Panic)
        .and_then(|result| result);
        if result.is_err() {
            *self = Self::default();
        }
        result
    }
}

/// Keeps preview uploads inexpensive and separate from the full-resolution still used for capture.
/// The caller supplies a transient texture identity independent of the original.
pub fn preview(frame: RgbaImage, key: blake3::Hash, format: ImageFormat) -> Decoded {
    let [w, h] = [frame.width(), frame.height()];
    let longer = w.max(h);
    let frame = if longer > PREVIEW_SIDE {
        image::imageops::thumbnail(
            &frame,
            (w * PREVIEW_SIDE / longer).max(1),
            (h * PREVIEW_SIDE / longer).max(1),
        )
    } else {
        frame
    };
    // BC7 cannot represent a mip thinner than a block. The registry still sees
    // the original aspect through the item's geometry.
    let frame = if format == ImageFormat::Bc7 && (frame.width() < 4 || frame.height() < 4) {
        image::imageops::resize(
            &frame,
            frame.width().max(4),
            frame.height().max(4),
            image::imageops::FilterType::Nearest,
        )
    } else {
        frame
    };
    crate::apply_format(format, crate::pyramid(frame, key, [w, h]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor, Write};

    struct Original(&'static [u8]);

    impl Storage for Original {
        fn open(&self, _: Handle) -> io::Result<Box<dyn moth_format::ReadSeek>> {
            Ok(Box::new(Cursor::new(self.0)))
        }

        fn append(&self, _: Handle) -> io::Result<Box<dyn Write + Send>> {
            Err(io::ErrorKind::Unsupported.into())
        }

        fn sync(&self, _: Handle) -> io::Result<()> {
            Err(io::ErrorKind::Unsupported.into())
        }
    }

    #[test]
    fn gif_delays_and_composited_pixels_survive_seeking_backwards() {
        let original = Original(include_bytes!("../tests/fixtures/animated.gif"));
        let handle = Handle(1);
        let delays = timeline(&original, handle).unwrap().unwrap();
        assert_eq!(delays, [20, 70, 110, 100].map(Duration::from_millis));
        assert!(timeline(&Original(b"PNG..."), handle).unwrap().is_none());

        let mut reader = Reader::default();
        let pixels = |frame: RgbaImage| frame.pixels().map(|pixel| pixel.0).collect::<Vec<_>>();
        let red = [255, 0, 0, 255];
        let green = [0, 255, 0, 255];
        let blue = [0, 0, 255, 255];
        assert_eq!(
            pixels(reader.frame(&original, handle, 2).unwrap()),
            [red, red, blue]
        );
        assert_eq!(
            pixels(reader.frame(&original, handle, 3).unwrap()),
            [green, red, [0; 4]]
        );
        assert_eq!(
            pixels(reader.frame(&original, handle, 1).unwrap()),
            [red, green, red]
        );
        assert!(reader.frame(&original, handle, 4).is_err());
    }
}
