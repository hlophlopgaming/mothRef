// SPDX-License-Identifier: GPL-3.0-or-later
//! AVIF's whole-frame codec shares the admission gate and retained frame with WebP.
use crate::DecodeError;
#[cfg(not(target_arch = "wasm32"))]
use image::DynamicImage;
use std::sync::Arc;

// shortcut: AV1 retains reference planes, so cap at 32 Mi pixels until a reduced-output codec exists.
const MAX_PIXELS: u64 = 32 * 1024 * 1024;

fn error(e: impl std::fmt::Display) -> DecodeError {
    DecodeError::Avif(e.to_string())
}

pub(super) fn dimensions(bytes: &[u8]) -> Result<[u32; 2], DecodeError> {
    if bytes.len() as u64 > crate::MAX_WEBP_INPUT_BYTES {
        return Err(DecodeError::Budget);
    }
    validate_boxes(bytes)?;
    let _metadata = crate::frames::admit(bytes.len() as u64 * 16);
    let parsed = avif_parse::read_avif(&mut std::io::Cursor::new(bytes)).map_err(error)?;
    let size = sequence_dimensions(&parsed.primary_item)?;
    if let Some(alpha) = parsed.alpha_item.as_ref()
        && sequence_dimensions(alpha)? != size
    {
        return Err(DecodeError::Budget);
    }
    Ok(size)
}

fn validate_boxes(bytes: &[u8]) -> Result<(), DecodeError> {
    validate_box_level(bytes, 0, None)
}

fn validate_box_level(
    mut bytes: &[u8],
    depth: u8,
    reference_width: Option<usize>,
) -> Result<(), DecodeError> {
    if depth > 8 {
        return Err(DecodeError::Budget);
    }
    // The parser reserves declared mdat sizes and asserts on truncated boxes in debug builds.
    while !bytes.is_empty() {
        let header = bytes.get(..8).ok_or(DecodeError::Budget)?;
        let size = u32::from_be_bytes(header[..4].try_into().map_err(|_| DecodeError::Budget)?);
        let (size, mut header_len) = match size {
            0 => (bytes.len(), 8),
            1 => {
                let size = u64::from_be_bytes(
                    bytes
                        .get(8..16)
                        .ok_or(DecodeError::Budget)?
                        .try_into()
                        .map_err(|_| DecodeError::Budget)?,
                );
                (usize::try_from(size).map_err(|_| DecodeError::Budget)?, 16)
            }
            size => (size as usize, 8),
        };
        if &header[4..8] == b"uuid" {
            header_len += 16;
        }
        if size < header_len || size > bytes.len() {
            return Err(DecodeError::Budget);
        }
        let body = &bytes[header_len..size];
        if let Some(width) = reference_width {
            let count = body.get(width..width + 2).ok_or(DecodeError::Budget)?;
            let count = usize::from(u16::from_be_bytes([count[0], count[1]]));
            if body.len() != width + 2 + count * width {
                return Err(DecodeError::Budget);
            }
        } else {
            match &header[4..8] {
                b"meta" => {
                    if body.len() > 8 * 1024 * 1024 {
                        return Err(DecodeError::Budget);
                    }
                    validate_box_level(body.get(4..).ok_or(DecodeError::Budget)?, depth + 1, None)?;
                }
                b"iprp" | b"ipco" => validate_box_level(body, depth + 1, None)?,
                b"iinf" => {
                    let version = *body.first().ok_or(DecodeError::Budget)?;
                    let prefix = if version == 0 { 6 } else { 8 };
                    let count = body
                        .get(4..prefix)
                        .ok_or(DecodeError::Budget)?
                        .iter()
                        .fold(0usize, |n, &byte| n * 256 + usize::from(byte));
                    let entries = body.get(prefix..).ok_or(DecodeError::Budget)?;
                    if count > entries.len() / 8 {
                        return Err(DecodeError::Budget);
                    }
                    validate_box_level(entries, depth + 1, None)?;
                }
                b"iref" => {
                    let version = *body.first().ok_or(DecodeError::Budget)?;
                    let width = if version == 0 { 2 } else { 4 };
                    validate_box_level(
                        body.get(4..).ok_or(DecodeError::Budget)?,
                        depth + 1,
                        Some(width),
                    )?;
                }
                b"pitm" => {
                    let version = *body.first().ok_or(DecodeError::Budget)?;
                    if body.len() != if version == 0 { 6 } else { 8 } {
                        return Err(DecodeError::Budget);
                    }
                }
                b"pixi" => {
                    let channels = usize::from(*body.get(4).ok_or(DecodeError::Budget)?);
                    if channels > 16 || body.len() != 5 + channels {
                        return Err(DecodeError::Budget);
                    }
                }
                b"ipma" => {
                    let fields = body.get(..8).ok_or(DecodeError::Budget)?;
                    let width = if fields[0] == 0 { 2 } else { 4 };
                    let association_width = if fields[3] & 1 == 0 { 1 } else { 2 };
                    let count = u32::from_be_bytes(
                        fields[4..8].try_into().map_err(|_| DecodeError::Budget)?,
                    );
                    let mut entries = &body[8..];
                    if count as usize > entries.len() / (width + 1) {
                        return Err(DecodeError::Budget);
                    }
                    for _ in 0..count {
                        let associations =
                            usize::from(*entries.get(width).ok_or(DecodeError::Budget)?);
                        entries = entries
                            .get(width + 1 + associations * association_width..)
                            .ok_or(DecodeError::Budget)?;
                    }
                    if !entries.is_empty() {
                        return Err(DecodeError::Budget);
                    }
                }
                _ => {}
            }
        }
        bytes = &bytes[size..];
    }
    Ok(())
}

fn sequence_dimensions(mut bytes: &[u8]) -> Result<[u32; 2], DecodeError> {
    let mut size = None;
    // Validate every sequence header: a later one must not bypass the allocation budget.
    while let Some((&header, rest)) = bytes.split_first() {
        let obu = bytes;
        if header & 0x81 != 0 {
            return Err(DecodeError::Budget);
        }
        bytes = rest;
        if header & 4 != 0 {
            bytes = bytes.get(1..).ok_or(DecodeError::Budget)?;
        }
        let length = if header & 2 != 0 {
            let mut length = 0u64;
            let mut complete = false;
            for shift in (0..56).step_by(7) {
                let (&byte, rest) = bytes.split_first().ok_or(DecodeError::Budget)?;
                bytes = rest;
                length |= u64::from(byte & 127) << shift;
                if byte & 128 == 0 {
                    complete = true;
                    break;
                }
            }
            if !complete {
                return Err(DecodeError::Budget);
            }
            usize::try_from(length).map_err(|_| DecodeError::Budget)?
        } else {
            bytes.len()
        };
        if length > bytes.len() {
            return Err(DecodeError::Budget);
        }
        if (header >> 3) & 15 == 1 {
            let metadata = avif_parse::AV1Metadata::parse_av1_bitstream(obu).map_err(error)?;
            let next = [
                metadata.max_frame_width.get(),
                metadata.max_frame_height.get(),
            ];
            if !metadata.still_picture
                || next.iter().any(|&side| side > crate::MAX_DIMENSION)
                || u64::from(next[0]) * u64::from(next[1]) > MAX_PIXELS
                || size.is_some_and(|size| size != next)
            {
                return Err(DecodeError::Budget);
            }
            size = Some(next);
        }
        bytes = &bytes[length..];
    }
    size.ok_or(DecodeError::Budget)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn frame(bytes: &[u8]) -> Result<Arc<image::RgbaImage>, DecodeError> {
    let [w, h] = dimensions(bytes)?;
    let decode = || {
        // Include high-bit-depth reference planes, RGB conversion and the input copies.
        let _frame =
            crate::frames::admit(u64::from(w) * u64::from(h) * 24 + bytes.len() as u64 * 16);
        let decoded = avif_decode::Decoder::from_avif(bytes)
            .and_then(avif_decode::Decoder::to_image)
            .map_err(error)?;
        macro_rules! pixels {
            ($p:ident, $variant:ident) => {{
                DynamicImage::$variant(
                    image::ImageBuffer::from_raw(
                        $p.width() as u32,
                        $p.height() as u32,
                        bytemuck::cast_slice($p.buf()).to_vec(),
                    )
                    .ok_or(DecodeError::Budget)?,
                )
            }};
        }
        let image = match decoded {
            avif_decode::Image::Rgb8(p) => pixels!(p, ImageRgb8),
            avif_decode::Image::Rgba8(p) => pixels!(p, ImageRgba8),
            avif_decode::Image::Gray8(p) => pixels!(p, ImageLuma8),
            avif_decode::Image::Rgb16(p) => pixels!(p, ImageRgb16),
            avif_decode::Image::Rgba16(p) => pixels!(p, ImageRgba16),
            avif_decode::Image::Gray16(p) => pixels!(p, ImageLuma16),
        };
        if image.width() != w || image.height() != h {
            return Err(DecodeError::Budget);
        }
        Ok(image.into_rgba8())
    };
    if u64::from(w) * u64::from(h) * 4 >= crate::frames::SHARED_FROM {
        crate::frames::shared(crate::content_hash(bytes), decode)
    } else {
        Ok(Arc::new(decode()?))
    }
}

// shortcut: browser AVIF needs the Worker codec in M8, as native libaom uses C.
#[cfg(target_arch = "wasm32")]
pub(super) fn frame(_bytes: &[u8]) -> Result<Arc<image::RgbaImage>, DecodeError> {
    Err(DecodeError::Budget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn opaque_and_alpha_avif_keep_pixels_through_previews_and_tiles() {
        for (bytes, alpha) in [
            (
                include_bytes!("../../../corpus/images/still.avif").as_slice(),
                255,
            ),
            (
                include_bytes!("../../../corpus/images/alpha.avif").as_slice(),
                128,
            ),
        ] {
            assert_eq!(dimensions(bytes).unwrap(), [31, 17]);
            let preview = crate::decode(bytes, 64).unwrap();
            assert_eq!(preview.original, [31, 17]);
            assert_eq!(preview.hash, crate::content_hash(bytes));
            for pixel in preview.mips[0].bytes.as_chunks::<4>().0 {
                for (actual, expected) in pixel.iter().zip([120, 90, 60, alpha]) {
                    assert!(actual.abs_diff(expected) <= 2);
                }
            }
            let tile = crate::tiles::Tile {
                level: 0,
                x: 0,
                y: 0,
            };
            let decoded = crate::tiles::decode(bytes, tile, &AtomicBool::new(false)).unwrap();
            assert_eq!(decoded.original, preview.original);
            assert_eq!(decoded.hash, preview.hash);
            assert_eq!(&decoded.mips[0].bytes[..4], &preview.mips[0].bytes[..4]);
            let mut baked = 0;
            crate::tiles::bake(bytes, 0..1, &AtomicBool::new(false), &mut |image| {
                assert_eq!(image.mips[0].bytes, decoded.mips[0].bytes);
                baked += 1;
                Ok(())
            })
            .unwrap();
            assert_eq!(baked, 1);
            assert!(matches!(
                crate::tiles::decode(bytes, tile, &AtomicBool::new(true)),
                Err(DecodeError::Cancelled)
            ));
            assert!(crate::decode(&bytes[..bytes.len() / 2], 64).is_err());
        }
    }

    #[test]
    fn invalid_containers_fail_without_unwinding() {
        for bytes in [b"".as_slice(), b"\0\0\0\x18ftypavif\0\0\0\0avifmif1"] {
            assert!(dimensions(bytes).is_err());
            assert!(crate::decode(bytes, 64).is_err());
            assert!(crate::tiles::dimensions(bytes).is_err());
        }
    }

    #[test]
    fn boxes_cannot_claim_bytes_past_the_input() {
        let bytes = b"\0\0\0\x10ftypavif\0\0\0\0\0\0\0\x10free";
        assert!(matches!(dimensions(bytes), Err(DecodeError::Budget)));
        for bytes in [
            b"\0\0\0\x01free".as_slice(),
            b"\0\0\0\x07free",
            b"\0\0\0\x01free\xff\xff\xff\xff\xff\xff\xff\xff",
        ] {
            assert!(validate_boxes(bytes).is_err());
        }
        assert!(validate_boxes(b"\0\0\0\0free").is_ok());
        assert!(validate_boxes(b"\0\0\0\x01free\0\0\0\0\0\0\0\x10").is_ok());
    }

    #[test]
    fn nested_property_lengths_and_counts_are_checked_before_parsing() {
        let original = include_bytes!("../../../corpus/images/alpha.avif");
        let mut oversized = original.to_vec();
        oversized[262] = 0;
        oversized[264] = 17;
        assert!(matches!(dimensions(&oversized), Err(DecodeError::Budget)));
        for kind in [b"pixi", b"ipma"] {
            let mut bytes = original.to_vec();
            let at = bytes.windows(4).position(|name| name == kind).unwrap();
            if kind == b"pixi" {
                bytes[at + 8] = 255;
            } else {
                bytes[at + 8..at + 12].fill(0);
            }
            assert!(matches!(dimensions(&bytes), Err(DecodeError::Budget)));
        }
        let mut nested = Vec::new();
        for _ in 0..10 {
            let mut parent = ((nested.len() + 12) as u32).to_be_bytes().to_vec();
            parent.extend_from_slice(b"meta\0\0\0\0");
            parent.extend_from_slice(&nested);
            nested = parent;
        }
        assert!(validate_boxes(&nested).is_err());
    }

    #[test]
    fn trailing_sequence_headers_and_malformed_obu_lengths_are_validated() {
        let parsed = avif_parse::read_avif(
            &mut include_bytes!("../../../corpus/images/still.avif").as_slice(),
        )
        .unwrap();
        let mut payload = parsed.primary_item.to_vec();
        assert_eq!(sequence_dimensions(&payload).unwrap(), [31, 17]);
        payload.extend_from_slice(&[0x0a, 1, 0xff]);
        assert!(sequence_dimensions(&payload).is_err());
        for bytes in [
            &[0x0a, 0x80][..],
            &[0x0a, 255, 255, 255, 255, 255, 255, 255, 255],
            &[0x0a, 127],
        ] {
            assert!(sequence_dimensions(bytes).is_err());
        }
    }
}
