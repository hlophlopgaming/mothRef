// SPDX-License-Identifier: MIT AND GPL-3.0-or-later
// Copyright (c) 2026 achrllrogia45
// Source: https://github.com/achrllrogia45/LazyRef
// Derived from achrllrogia45/LazyRef, commit e59ff1fb63a7ad3b4c6d52b0ab2de540e957b654.
// Original copyright and permission notice: ../LICENSE.
//! Legacy v1.x `.pur` reader — the pre-2.0 binary format ("1.10", 224-byte header).
//!
//! Read-only by design (there is no legacy writer). Port of the reference reader
//! `PureRef-format/purformat/read.py`, field-for-field cross-checked in tests against the
//! dump in `tests/fixtures/legacy-dumps/S1_redbar.tsv`.
//!
//! ## Mapping decision: dedicated model, not [`crate::model::Scene`]
//! The legacy format predates the SQLite container: no groups/notes/drawings/metadata
//! tables, and transforms nest under images with crop-point lists and text children.
//! This reader returns [`LegacyScene`]; the app layer maps it to the board directly.
//!
//! ## Deliberate deviations from the Python reference (malformed input only)
//! Structurally broken files return errors; nesting, item counts and encoded size
//! have explicit limits. Well-formed files retain the upstream interpretation. Checksum
//! verification mirrors the container path: `stored == md5(file[108:])` hex UTF-16BE at
//! `[44:108]`.

use crate::{
    be,
    sqlite::ReadSeek,
    stream::{Entry, Error, verify_checksum},
};
use std::io::SeekFrom;

const PNG_HEAD: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
const PNG_FOOT: [u8; 12] = [0, 0, 0, 0, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82];
const TAG_IMAGE_ITEM: u32 = 34;
const TAG_TEXT_ITEM: u32 = 32;
const HEADER_LENGTH: u64 = 224;

pub(crate) fn read<S: ReadSeek>(
    mut source: S,
    length: u64,
    emit: &mut impl FnMut(Entry<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    require!(length >= HEADER_LENGTH);
    let mut header = [0; 224];
    source.seek(SeekFrom::Start(0))?;
    source.read_exact(&mut header)?;
    verify_checksum(&mut source, length, 108, &header[44..108])?;
    let stored = be::utf16_be_to_string(&header[44..108]);
    let mut parser = Parser {
        source,
        length,
        pos: 0,
        depth: 0,
        items: 0,
        metadata_bytes: 0,
    };
    let mut scene = parser.parse(stored.clone(), stored)?;
    // Metadata comes after the blobs. Resolve references first, then fetch each
    // original separately rather than retaining all image payloads in RAM.
    let images = std::mem::take(&mut scene.images);
    let texts = std::mem::take(&mut scene.texts);
    for mut image in images {
        if image.png.is_empty() {
            let bytes = image
                .address_end
                .checked_sub(image.address_start)
                .ok_or(Error::Invalid)?;
            require!(bytes <= 32 * 1024 * 1024);
            image.png = parser.bytes_at(image.address_start, bytes as usize)?;
        }
        scene.images.push(image);
        emit(Entry::Legacy(&scene))?;
        scene.images.clear();
    }
    scene.texts = texts;
    if !scene.texts.is_empty() {
        emit(Entry::Legacy(&scene))?;
    }
    Ok(())
}

struct Parser<S> {
    source: S,
    length: u64,
    pos: u64,
    depth: usize,
    items: usize,
    metadata_bytes: usize,
}

impl<S: ReadSeek> Parser<S> {
    fn parse(
        &mut self,
        stored_checksum: String,
        computed_checksum: String,
    ) -> Result<LegacyScene, crate::Error> {
        let canvas = [
            self.f64_at(112)?,
            self.f64_at(120)?,
            self.f64_at(128)?,
            self.f64_at(136)?,
        ];
        let zoom = self.f64_at(144)?;
        let x_canvas = self.i32_at(216)?;
        let y_canvas = self.i32_at(220)?;
        self.pos = HEADER_LENGTH;

        let mut images = self.read_images()?;
        let mut image_items: Vec<LegacyImageItem> = Vec::new();
        let mut texts: Vec<LegacyTextItem> = Vec::new();
        loop {
            // item area: tag lives at offset 8 (after the u64 end address)
            self.need(self.pos + 12)?;
            let tag = self.u32_at(self.pos + 8)?;
            if tag == TAG_IMAGE_ITEM {
                image_items.push(self.read_image_item()?);
            } else if tag == TAG_TEXT_ITEM {
                texts.push(self.read_text_item()?);
            } else {
                break;
            }
        }

        let folder_location = self.read_string()?;
        let count = image_items.len();
        let mut items = std::collections::HashMap::new();
        for item in image_items {
            require!(items.insert(item.id, item).is_none());
        }
        let mut addresses = std::collections::HashMap::new();
        for (index, image) in images.iter().enumerate() {
            addresses.insert(image.address_start, index);
        }
        for _ in 0..count {
            self.need(self.pos + 20)?;
            let id = self.u32_at(self.pos)?;
            let begin = self.u64_at(self.pos + 4)?;
            self.pos += 20;
            let item = items.remove(&id).ok_or(crate::Error::Invalid)?;
            let index = *addresses.get(&begin).ok_or(crate::Error::Invalid)?;
            images[index].transforms.push(item);
        }
        let parents: std::collections::HashMap<_, _> = images
            .iter()
            .enumerate()
            .filter(|(_, image)| !is_duplicate(image))
            .flat_map(|(index, image)| image.transforms.iter().map(move |item| (item.id, index)))
            .collect();
        for index in 0..images.len() {
            if is_duplicate(&images[index]) {
                let id = be::u32(&images[index].png, 0)?;
                let parent = *parents.get(&id).ok_or(crate::Error::Invalid)?;
                let transforms = std::mem::take(&mut images[index].transforms);
                images[parent].transforms.extend(transforms);
            }
        }
        Ok(LegacyScene {
            canvas,
            zoom,
            x_canvas,
            y_canvas,
            folder_location,
            images: images
                .into_iter()
                .filter(|img| !is_duplicate(img))
                .collect(),
            texts,
            stored_checksum,
            computed_checksum,
        })
    }

    /// Image area: PNGs located by magic; every 4 bytes before a PNG (or before the items)
    /// is a duplicate.
    fn read_images(&mut self) -> Result<Vec<LegacyImage>, crate::Error> {
        let mut images = Vec::new();
        let mut next_png = self.index_of(&PNG_HEAD, self.pos)?;
        loop {
            let start = next_png;
            if start < 0 {
                break;
            }
            let start = start as u64;
            if start - self.pos >= 4 {
                self.need(self.pos + 4)?;
                require!(images.len() < 8192);
                self.charge(128)?;
                images.push(LegacyImage {
                    address_start: self.pos,
                    address_end: self.pos + 4,
                    png: self.bytes_at(self.pos, 4)?,
                    transforms: Vec::new(),
                });
                self.pos += 4;
            } else {
                let foot = self.index_of(&PNG_FOOT, self.pos)?;
                require!(foot >= 0 && foot as u64 >= start);
                let foot = foot as u64;
                // consumed length, relative to pos ([start] is absolute)
                let len = foot + PNG_FOOT.len() as u64 - self.pos;
                require!(images.len() < 8192);
                self.charge(128)?;
                images.push(LegacyImage {
                    address_start: start,
                    address_end: self.pos + len,
                    png: Vec::new(),
                    transforms: Vec::new(),
                });
                self.pos += len;
                next_png = self.index_of(&PNG_HEAD, self.pos)?;
            }
        }
        loop {
            // trailing duplicates between the last PNG and the first item
            self.need(self.pos + 12)?;
            let tag = self.u32_at(self.pos + 8)?;
            if tag == TAG_IMAGE_ITEM || tag == TAG_TEXT_ITEM {
                break;
            }
            self.need(self.pos + 4)?;
            require!(images.len() < 8192);
            self.charge(128)?;
            images.push(LegacyImage {
                address_start: self.pos,
                address_end: self.pos + 4,
                png: self.bytes_at(self.pos, 4)?,
                transforms: Vec::new(),
            });
            self.pos += 4;
        }
        Ok(images)
    }

    fn read_text_item(&mut self) -> Result<LegacyTextItem, crate::Error> {
        self.item_budget()?;
        require!(self.depth < 32);
        self.depth += 1;
        let transform_end = self.end_address()?;
        self.pos += 12 + self.tag_length()?; // tag + "GraphicsTextItem" (tag == byte length)
        let text = self.read_string()?;
        let matrix = self.read_matrix()?;
        let x = self.read_double()?;
        let y = self.read_double()?;
        self.pos += 8; // permanent 1.0 double
        let id = self.u32_at(self.pos)?;
        self.pos += 4;
        let z_layer = self.read_double()?;
        let hsv_foreground = self.i8_at(self.pos)? == 2;
        self.pos += 1;
        let opacity = self.u16_at(self.pos)?;
        self.pos += 2;
        let mut rgb = [
            self.u16_at(self.pos)? as i32,
            self.u16_at(self.pos + 2)? as i32,
            self.u16_at(self.pos + 4)? as i32,
        ];
        self.pos += 6;
        if hsv_foreground {
            rgb = hsv_to_rgb(rgb);
        }
        self.pos += 2; // unknown
        let hsv_background = self.i8_at(self.pos)? == 2;
        self.pos += 1;
        let opacity_background = self.u16_at(self.pos)?;
        self.pos += 2;
        let mut rgb_background = [
            self.u16_at(self.pos)? as i32,
            self.u16_at(self.pos + 2)? as i32,
            self.u16_at(self.pos + 4)? as i32,
        ];
        self.pos += 6;
        if hsv_background {
            rgb_background = hsv_to_rgb(rgb_background);
        }
        let children_count = self.u32_at(self.pos + 2)?; // 2-byte unknown, then u32 count
        self.seek_to(transform_end)?;
        let text_children = self.read_text_children(children_count)?;
        self.depth -= 1;
        Ok(LegacyTextItem {
            text,
            matrix,
            x,
            y,
            id,
            z_layer,
            opacity: opacity as i32,
            rgb,
            opacity_background: opacity_background as i32,
            rgb_background,
            text_children,
        })
    }

    fn read_image_item(&mut self) -> Result<LegacyImageItem, crate::Error> {
        self.item_budget()?;
        let transform_end = self.end_address()?;
        self.pos += 12 + self.tag_length()?; // tag + "GraphicsImageItem"
        let mut source = "BruteForceLoaded".to_string(); // kept when the file stores -1 (absent)
        let mut name = "image".to_string();
        let brute_force_loaded = self.u32_at(self.pos)? == 0;
        if brute_force_loaded {
            self.pos += 4;
        }
        if self.u32_at(self.pos)? == 0xFFFF_FFFF {
            self.pos += 4;
        } else {
            source = self.read_string()?;
        }
        if !brute_force_loaded {
            if self.u32_at(self.pos)? == 0xFFFF_FFFF {
                self.pos += 4;
            } else {
                name = self.read_string()?;
            }
        }
        self.pos += 8; // permanent 1.0 double
        let matrix = self.read_matrix()?;
        let x = self.read_double()?;
        let y = self.read_double()?;
        self.pos += 8; // second permanent 1.0 double
        let id = self.u32_at(self.pos)?;
        self.pos += 4;
        let z_layer = self.read_double()?;
        let matrix_before_crop = self.read_matrix()?;
        let x_crop = self.read_double()?;
        let y_crop = self.read_double()?;
        let scale_crop = self.read_double()?;
        let point_count = self.u32_at(self.pos)? as usize;
        self.pos += 4;
        require!(point_count as u64 <= self.length.saturating_sub(self.pos) / 20);
        self.charge(point_count.checked_mul(16).ok_or(Error::Invalid)?)?;
        let mut points_x = Vec::with_capacity(point_count);
        let mut points_y = Vec::with_capacity(point_count);
        for _ in 0..point_count {
            self.pos += 4; // per-point flag
            points_x.push(self.read_double()?);
            points_y.push(self.read_double()?);
        }
        let children_count = self.u32_at(self.pos + 21)?; // fixed 25-byte tail, count last
        self.seek_to(transform_end)?;
        let text_children = self.read_text_children(children_count)?;
        Ok(LegacyImageItem {
            source,
            name,
            matrix,
            x,
            y,
            id,
            z_layer,
            matrix_before_crop,
            x_crop,
            y_crop,
            scale_crop,
            points_x,
            points_y,
            text_children,
        })
    }

    fn read_text_children(&mut self, count: u32) -> Result<Vec<LegacyTextItem>, crate::Error> {
        require!(count <= 8192);
        let mut children = Vec::new();
        let mut i = 0u32;
        while i < count {
            // u32 bound: a corrupt count must not wrap
            children.push(self.read_text_item()?);
            i += 1;
        }
        Ok(children)
    }

    // ------------------------------------------------------------------ primitives

    fn item_budget(&mut self) -> Result<(), crate::Error> {
        self.charge(512)?;
        self.items += 1;
        require!(self.items <= 8192);
        Ok(())
    }

    fn end_address(&mut self) -> Result<u64, crate::Error> {
        let end = self.u64_at(self.pos)?;
        require!(end >= self.pos && end <= self.length);
        Ok(end)
    }

    /// Item type tag at offset 8, also the UTF-16 byte length of the "Graphics…Item" string.
    fn tag_length(&mut self) -> Result<u64, crate::Error> {
        let len = self.u32_at(self.pos + 8)?;
        require!(self.pos + 12 + len as u64 <= self.length);
        Ok(len as u64)
    }

    fn seek_to(&mut self, end: u64) -> Result<(), crate::Error> {
        require!(end >= self.pos);
        self.pos = end;
        Ok(())
    }

    fn read_string(&mut self) -> Result<String, crate::Error> {
        let len = self.u32_at(self.pos)? as usize;
        self.pos += 4;
        require!(len as u64 <= self.length.saturating_sub(self.pos));
        self.charge(len.checked_mul(2).ok_or(Error::Invalid)?)?;
        let s = be::utf16_be_to_string(&self.bytes_at(self.pos, len)?);
        self.pos += len as u64;
        Ok(s)
    }

    fn read_double(&mut self) -> Result<f64, crate::Error> {
        let v = self.f64_at(self.pos)?;
        self.pos += 8;
        Ok(v)
    }

    /// The 6×f64 matrix carriers: m0, m1, (skip 1.0), m2, m3, (skip 1.0).
    fn read_matrix(&mut self) -> Result<[f64; 4], crate::Error> {
        let m = [
            self.f64_at(self.pos)?,
            self.f64_at(self.pos + 8)?,
            self.f64_at(self.pos + 24)?,
            self.f64_at(self.pos + 32)?,
        ];
        self.pos += 48;
        Ok(m)
    }

    fn f64_at(&mut self, offset: u64) -> Result<f64, Error> {
        let mut bytes = [0u8; 8];
        require!(offset.checked_add(8).is_some_and(|end| end <= self.length));
        self.source.seek(SeekFrom::Start(offset))?;
        self.source.read_exact(&mut bytes)?;
        be::f64(&bytes, 0)
    }
    fn i32_at(&mut self, offset: u64) -> Result<i32, Error> {
        let mut bytes = [0u8; 4];
        require!(offset.checked_add(4).is_some_and(|end| end <= self.length));
        self.source.seek(SeekFrom::Start(offset))?;
        self.source.read_exact(&mut bytes)?;
        be::i32(&bytes, 0)
    }
    fn u32_at(&mut self, offset: u64) -> Result<u32, Error> {
        let mut bytes = [0u8; 4];
        require!(offset.checked_add(4).is_some_and(|end| end <= self.length));
        self.source.seek(SeekFrom::Start(offset))?;
        self.source.read_exact(&mut bytes)?;
        be::u32(&bytes, 0)
    }
    fn u64_at(&mut self, offset: u64) -> Result<u64, Error> {
        let mut bytes = [0u8; 8];
        require!(offset.checked_add(8).is_some_and(|end| end <= self.length));
        self.source.seek(SeekFrom::Start(offset))?;
        self.source.read_exact(&mut bytes)?;
        be::u64(&bytes, 0)
    }
    fn u16_at(&mut self, offset: u64) -> Result<u16, Error> {
        let mut bytes = [0u8; 2];
        require!(offset.checked_add(2).is_some_and(|end| end <= self.length));
        self.source.seek(SeekFrom::Start(offset))?;
        self.source.read_exact(&mut bytes)?;
        be::u16(&bytes, 0)
    }
    fn i8_at(&mut self, offset: u64) -> Result<i8, Error> {
        let mut bytes = [0u8; 1];
        require!(offset.checked_add(1).is_some_and(|end| end <= self.length));
        self.source.seek(SeekFrom::Start(offset))?;
        self.source.read_exact(&mut bytes)?;
        be::i8(&bytes, 0)
    }
    fn index_of(&mut self, pattern: &[u8], from: u64) -> Result<i64, Error> {
        require!(!pattern.is_empty() && pattern.len() <= 12);
        self.source.seek(SeekFrom::Start(from))?;
        let mut buffer = [0u8; 65548];
        let mut carry = 0;
        let mut offset = from;
        loop {
            let remaining = self.length.saturating_sub(offset + carry as u64);
            let count = remaining.min(65536) as usize;
            if count == 0 {
                return Ok(-1);
            }
            let n = self.source.read(&mut buffer[carry..carry + count])?;
            if n == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into());
            }
            let end = carry + n;
            if let Some(index) = buffer[..end]
                .windows(pattern.len())
                .position(|w| w == pattern)
            {
                return Ok((offset + index as u64) as i64);
            }
            carry = end.min(pattern.len() - 1);
            buffer.copy_within(end - carry..end, 0);
            offset += (end - carry) as u64;
        }
    }
    fn bytes_at(&mut self, offset: u64, length: usize) -> Result<Vec<u8>, Error> {
        require!(
            offset
                .checked_add(length as u64)
                .is_some_and(|end| end <= self.length)
        );
        let mut bytes = vec![0; length];
        self.source.seek(SeekFrom::Start(offset))?;
        self.source.read_exact(&mut bytes)?;
        Ok(bytes)
    }
    fn charge(&mut self, bytes: usize) -> Result<(), Error> {
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(bytes)
            .ok_or(Error::Invalid)?;
        require!(self.metadata_bytes <= 8 * 1024 * 1024);
        Ok(())
    }

    fn need(&self, off: u64) -> Result<(), crate::Error> {
        require!(off <= self.length);
        Ok(())
    }
}

fn is_duplicate(img: &LegacyImage) -> bool {
    img.png.len() == 4 && !img.png.iter().all(|b| *b == 0xFF)
}

/// `colorsys.hsv_to_rgb(h/35900, s/65535, v/65535)` then `int(x*65535)` — Python's exact
/// arithmetic order, so results match bit-for-bit (verified by the L3-edge fixture).
fn hsv_to_rgb(hsv: [i32; 3]) -> [i32; 3] {
    colorsys_hsv_to_rgb(
        hsv[0] as f64 / 35900.0,
        hsv[1] as f64 / 65535.0,
        hsv[2] as f64 / 65535.0,
    )
    .map(|x| (x * 65535.0) as i32)
}

fn colorsys_hsv_to_rgb(h: f64, s: f64, v: f64) -> [f64; 3] {
    if s == 0.0 {
        return [v, v, v];
    }
    let i0 = (h * 6.0) as i32;
    let f = h * 6.0 - i0 as f64;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match ((i0 % 6) + 6) % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// A parsed legacy v1.x scene. Images carry their duplicate/link entries merged in (each
/// [`LegacyImage`] holds the original plus any duplicates as [`LegacyImageItem`]s); [`texts`]
/// are the top-level text items, each with a `text_children` tree.
///
/// [`texts`]: LegacyScene::texts
#[derive(Debug, Clone)]
pub struct LegacyScene {
    pub canvas: [f64; 4], // 4 doubles: x1, y1, x2, y2
    pub zoom: f64,
    pub x_canvas: i32,
    pub y_canvas: i32,
    pub folder_location: String,
    pub images: Vec<LegacyImage>,
    pub texts: Vec<LegacyTextItem>,
    pub stored_checksum: String,   // header [44:108], UTF-16BE
    pub computed_checksum: String, // md5(file[108:]) hex — what the desktop app enforces
}

impl LegacyScene {
    pub fn checksum_matches(&self) -> bool {
        self.stored_checksum == self.computed_checksum
    }
}

/// One image blob and the transforms that display it. `png` is the PNG file's bytes; for a
/// duplicate entry it is 4 bytes holding the parent transform id; an image link is 4 bytes
/// `FF FF FF FF` and is kept (unlike duplicates, which are merged and dropped).
#[derive(Debug, Clone)]
pub struct LegacyImage {
    pub address_start: u64, // file offsets of the blob
    pub address_end: u64,
    pub png: Vec<u8>,
    pub transforms: Vec<LegacyImageItem>, // assigned by the reference table, merged by dedup
}

/// Image transform: placement, crop rectangle and crop points.
#[derive(Debug, Clone)]
pub struct LegacyImageItem {
    pub source: String,
    pub name: String,
    pub matrix: [f64; 4], // 2x2 (m0, m1, m2, m3) scale/rotation
    pub x: f64,
    pub y: f64,
    pub id: u32, // referenced by the file's trailing ref table
    pub z_layer: f64,
    pub matrix_before_crop: [f64; 4],
    pub x_crop: f64,
    pub y_crop: f64,
    pub scale_crop: f64,
    pub points_x: Vec<f64>, // crop polygon; both lists have the same length
    pub points_y: Vec<f64>,
    pub text_children: Vec<LegacyTextItem>,
}

/// Text item (top-level or child). Colors are 16-bit RGB; HSV-encoded ones are converted.
#[derive(Debug, Clone)]
pub struct LegacyTextItem {
    pub text: String,
    pub matrix: [f64; 4],
    pub x: f64,
    pub y: f64,
    pub id: u32,
    pub z_layer: f64,
    pub opacity: i32, // u16
    pub rgb: [i32; 3],
    pub opacity_background: i32, // u16
    pub rgb_background: [i32; 3],
    pub text_children: Vec<LegacyTextItem>,
}

#[cfg(test)]
mod safety_tests {
    use super::*;
    #[test]
    fn nesting_and_child_count_are_checked_before_reading() {
        let mut parser = Parser {
            source: std::io::Cursor::new(&[]),
            length: 0,
            metadata_bytes: 0,
            pos: 0,
            depth: 32,
            items: 0,
        };
        assert!(parser.read_text_item().is_err());
        let mut parser = Parser {
            source: std::io::Cursor::new(&[]),
            length: 0,
            metadata_bytes: 0,
            pos: 0,
            depth: 0,
            items: 0,
        };
        assert!(parser.read_text_children(8193).is_err());
    }
    #[test]
    fn corrupt_checksums_return_errors_without_a_panic() {
        let mut bytes = include_bytes!("../../../corpus/pur/v1.10-redbar.pur").to_vec();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(
            read(
                std::io::Cursor::new(&bytes),
                bytes.len() as u64,
                &mut |_| Ok(())
            )
            .is_err()
        );
    }
    #[test]
    fn large_legacy_original_is_streamed_after_placement_resolution() {
        let original = include_bytes!("../../../corpus/pur/v1.10-redbar.pur");
        let footer = original
            .windows(PNG_FOOT.len())
            .position(|w| w == PNG_FOOT)
            .unwrap();
        let old_end = footer + PNG_FOOT.len();
        let padding = 17 * 1024 * 1024;
        // Add a valid private PNG ancillary chunk in memory; no new .pur file is written.
        let extra = padding + 12;
        let mut bytes = Vec::with_capacity(original.len() + extra);
        bytes.extend_from_slice(&original[..footer]);
        bytes.extend_from_slice(&(padding as u32).to_be_bytes());
        bytes.extend_from_slice(b"ruSt");
        bytes.resize(bytes.len() + padding, 0);
        bytes.extend_from_slice(&0x42a73564u32.to_be_bytes());
        bytes.extend_from_slice(&original[footer..]);
        let item_end = be::u64(original, old_end).unwrap() + extra as u64;
        bytes[old_end + extra..old_end + extra + 8].copy_from_slice(&item_end.to_be_bytes());
        let end = bytes.len();
        bytes[end - 8..].copy_from_slice(&((old_end + extra) as u64).to_be_bytes());
        let digest = crate::checksum::md5_hex_from(&bytes, 108);
        for (i, c) in digest.bytes().enumerate() {
            bytes[44 + i * 2] = 0;
            bytes[45 + i * 2] = c;
        }
        let mut images = 0;
        read(
            std::io::Cursor::new(&bytes),
            bytes.len() as u64,
            &mut |entry| {
                let Entry::Legacy(scene) = entry else {
                    panic!("expected legacy callback")
                };
                assert_eq!(scene.images.len(), 1);
                assert!(scene.texts.is_empty());
                assert_eq!(scene.images[0].png.len(), 157 + extra);
                assert_eq!(scene.images[0].transforms.len(), 1);
                images += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(images, 1);
    }
    #[test]
    fn png_search_crosses_buffer_boundaries() {
        for offset in [65530, 65535, 65536] {
            let mut bytes = vec![0u8; offset];
            bytes.extend_from_slice(&PNG_HEAD);
            let mut parser = Parser {
                length: bytes.len() as u64,
                source: std::io::Cursor::new(bytes),
                pos: 0,
                depth: 0,
                items: 0,
                metadata_bytes: 0,
            };
            assert_eq!(parser.index_of(&PNG_HEAD, 0).unwrap(), offset as i64);
            assert_eq!(parser.index_of(&PNG_FOOT, 0).unwrap(), -1);
        }
    }
}
