// SPDX-License-Identifier: GPL-3.0-or-later
//! Row-based previews avoid retaining full GIF/BMP frames.
use super::{AreaPreview, DecodeError};
use image::ImageDecoder;
use std::io::Cursor;

fn accumulator(w: u32, h: u32, side: u32) -> AreaPreview {
    let ratio = (side as f64 / f64::from(w.max(h))).min(1.);
    AreaPreview::new(
        w,
        h,
        (w as f64 * ratio).round().max(1.) as u32,
        (h as f64 * ratio).round().max(1.) as u32,
    )
}

pub(super) fn gif(
    bytes: &[u8],
    w: u32,
    h: u32,
    side: u32,
) -> Result<image::RgbaImage, DecodeError> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    options.set_memory_limit(gif::MemoryLimit::Bytes(
        std::num::NonZeroU64::new(8 * 1024 * 1024).expect("nonzero fixed decoder budget"),
    ));
    options.check_frame_consistency(true);
    let mut decoder = options.read_info(bytes)?;
    let frame = decoder.next_frame_info()?.ok_or(DecodeError::Budget)?;
    let (left, top, width, height, interlaced) = (
        u32::from(frame.left),
        u32::from(frame.top),
        u32::from(frame.width),
        u32::from(frame.height),
        frame.interlaced,
    );
    if width == 0 || height == 0 || left + width > w || top + height > h {
        return Err(DecodeError::Budget);
    }
    let mut average = accumulator(w, h, side);
    let mut row = vec![0; w as usize * 4];
    let passes: &[(u32, u32)] = if interlaced {
        &[(0, 8), (4, 8), (2, 4), (1, 2)]
    } else {
        &[(0, 1)]
    };
    for &(start, step) in passes {
        for y in (start..height).step_by(step as usize) {
            if !decoder.fill_buffer(&mut row[left as usize * 4..(left + width) as usize * 4])? {
                return Err(DecodeError::Budget);
            }
            average.row(top + y, 0, 1, &row, png::ColorType::Rgba)?;
        }
    }
    // A static preview uses the first frame; animation playback belongs to M4.
    let mut tail = [0; 4];
    if decoder.fill_buffer(&mut tail)? {
        return Err(DecodeError::Budget);
    }
    Ok(average.finish())
}

pub(super) fn bmp(
    bytes: &[u8],
    w: u32,
    h: u32,
    side: u32,
) -> Result<image::RgbaImage, DecodeError> {
    let u32_at = |offset: usize| -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(
            bytes
                .get(offset..offset + 4)
                .ok_or(DecodeError::Budget)?
                .try_into()
                .map_err(|_| DecodeError::Budget)?,
        ))
    };
    let offset = u32_at(10)? as usize;
    let header = u32_at(14)?;
    let (height_offset, depth_offset, top_down) = match header {
        12 => (20, 24, false),
        40 | 52 | 56 | 108 | 124 => (22, 28, (u32_at(22)? as i32) < 0),
        _ => return Err(DecodeError::Budget),
    };
    // shortcut: compressed BMPs need cross-row decoding; add a streaming path when required.
    if header != 12 && !matches!(u32_at(30)?, 0 | 3 | 6) {
        return Err(DecodeError::Budget);
    }
    let depth = u16::from_le_bytes(
        bytes
            .get(depth_offset..depth_offset + 2)
            .ok_or(DecodeError::Budget)?
            .try_into()
            .map_err(|_| DecodeError::Budget)?,
    );
    if !matches!(depth, 1 | 4 | 8 | 16 | 24 | 32)
        || offset < 14 + header as usize
        || offset > 64 * 1024
    {
        return Err(DecodeError::Budget);
    }
    let stride = (w as usize * depth as usize).div_ceil(32) * 4;
    let end = offset
        .checked_add(stride * h as usize)
        .ok_or(DecodeError::Budget)?;
    if end > bytes.len() {
        return Err(DecodeError::Budget);
    }
    // Reuse the codec's palette/bitfield conversion on one synthetic row at a time.
    let mut input = bytes[..offset].to_vec();
    input.resize(offset + stride, 0);
    let size = input.len() as u32;
    input[2..6].copy_from_slice(&size.to_le_bytes());
    if header == 12 {
        input[height_offset..height_offset + 2].copy_from_slice(&1u16.to_le_bytes());
    } else {
        input[height_offset..height_offset + 4].copy_from_slice(&1u32.to_le_bytes());
        input[34..38].copy_from_slice(&(stride as u32).to_le_bytes());
    }
    let mut average = accumulator(w, h, side);
    let mut row = vec![0; w as usize * 4];
    for y in 0..h {
        let source_y = if top_down { y } else { h - 1 - y };
        let start = offset + source_y as usize * stride;
        input[offset..].copy_from_slice(&bytes[start..start + stride]);
        let decoder = image::codecs::bmp::BmpDecoder::new(Cursor::new(&input))?;
        let color = match decoder.color_type() {
            image::ColorType::Rgb8 => png::ColorType::Rgb,
            image::ColorType::Rgba8 => png::ColorType::Rgba,
            _ => return Err(DecodeError::Budget),
        };
        let len = w as usize * color.samples();
        decoder.read_image(&mut row[..len])?;
        average.row(y, 0, 1, &row[..len], color)?;
    }
    Ok(average.finish())
}

pub(super) fn rgba(
    bytes: &[u8],
    w: u32,
    h: u32,
    side: u32,
) -> Result<image::RgbaImage, DecodeError> {
    let mut average = accumulator(w, h, side);
    for (y, row) in bytes.chunks_exact(w as usize * 4).enumerate() {
        average.row(y as u32, 0, 1, row, png::ColorType::Rgba)?;
    }
    Ok(average.finish())
}

// Extended WebP canvas dimensions must not hide larger VP8/VP8L allocation sizes.
pub(super) fn webp_dimensions(bytes: &[u8], w: u32, h: u32) -> Result<(), DecodeError> {
    let mut offset = 12usize;
    while offset < bytes.len() {
        let header = bytes.get(offset..offset + 8).ok_or(DecodeError::Budget)?;
        let len =
            u32::from_le_bytes(header[4..8].try_into().map_err(|_| DecodeError::Budget)?) as usize;
        let start = offset + 8;
        let end = start.checked_add(len).ok_or(DecodeError::Budget)?;
        let data = bytes.get(start..end).ok_or(DecodeError::Budget)?;
        let dimensions = match &header[..4] {
            b"VP8L" => {
                let packed = u32::from_le_bytes(
                    data.get(1..5)
                        .ok_or(DecodeError::Budget)?
                        .try_into()
                        .map_err(|_| DecodeError::Budget)?,
                );
                Some(((packed & 0x3fff) + 1, ((packed >> 14) & 0x3fff) + 1))
            }
            b"VP8 " => {
                let size = data.get(6..10).ok_or(DecodeError::Budget)?;
                Some((
                    u32::from(u16::from_le_bytes([size[0], size[1]]) & 0x3fff),
                    u32::from(u16::from_le_bytes([size[2], size[3]]) & 0x3fff),
                ))
            }
            _ => None,
        };
        if dimensions.is_some_and(|size| size != (w, h)) {
            return Err(DecodeError::Budget);
        }
        offset = end.checked_add(len % 2).ok_or(DecodeError::Budget)?;
    }
    Ok(())
}
