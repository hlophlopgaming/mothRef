// SPDX-License-Identifier: GPL-3.0-or-later
//! Versioned append-only records. A checksummed footer commits one record.
use crate::ReadSeek;
use moth_scene::{BlobId, Command, Content, Item, Rect, Scene, Transform, Uuid};
use std::{
    collections::HashMap,
    io::{self, SeekFrom, Write},
};

const HEADER: &[u8; 16] = b"MOTH\x05\0\0\0\0\0\0\0\0\0\0\0";
/// Schema-4 boards predate sampling and fixed-width notes.
const HEADER_V4: &[u8; 16] = b"MOTH\x04\0\0\0\0\0\0\0\0\0\0\0";
/// Schema-3 boards predate appearance and source names.
const HEADER_V3: &[u8; 16] = b"MOTH\x03\0\0\0\0\0\0\0\0\0\0\0";
/// Schema-2 boards predate notes; the reader still accepts them.
const HEADER_V2: &[u8; 16] = b"MOTH\x02\0\0\0\0\0\0\0\0\0\0\0";
/// Schema-1 boards predate content transforms; the reader still accepts them.
const HEADER_V1: &[u8; 16] = b"MOTH\x01\0\0\0\0\0\0\0\0\0\0\0";
const FOOTER: &[u8; 8] = b"MOTHEND1";
const FOOTER_SIZE: u64 = 96;
/// Bounds allocations even for malicious input lengths.
pub const MAX_RECORD: u64 = 256 * 1024 * 1024;
/// Longest accepted image command payload: the largest fractional index plus its fixed fields.
const MAX_COMMAND: u64 = 5219;
/// Bytes of fixed command fields after the tag, before a schema-2 insert's fractional index.
const INSERT_V2_FIXED: usize = 86;
/// Bytes of fixed fields before a text insert's fractional index: tag, id, u32 length,
/// rectangle and transform (the UTF-8 text sits between the length and the rectangle).
const INSERT_TEXT_FIXED: usize = 1 + 16 + 4 + 16 + 32;
/// Longest accepted text insert or SetText command (text plus a fractional index).
const MAX_TEXT_COMMAND: u64 =
    INSERT_TEXT_FIXED as u64 + moth_scene::MAX_NOTE_BYTES as u64 + 4096 + 1026;
/// Bytes of fixed command fields in a transform record.
const SET_TRANSFORM_LEN: usize = 1 + 16 + 21;
/// Distinguishes unsupported files from recoverable torn tails.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Storage failures must reach the UI without acknowledging a mutation.
    #[error("board I/O: {0}")]
    Io(#[from] io::Error),
    /// Version checks prevent accidental interpretation of future formats.
    #[error("invalid or unsupported MothRef board")]
    Invalid,
    /// Limits bound both disk records and metadata.
    #[error("board record exceeds budget")]
    Budget,
    /// Invalid scene commands never partially mutate a board.
    #[error(transparent)]
    Scene(#[from] moth_scene::SceneError),
}
/// Original bytes stay in storage; only their locations live in memory.
#[derive(Clone, Copy, Debug)]
pub struct Blob {
    /// Offset into the byte stream, independent of native paths.
    pub offset: u64,
    /// Original encoded length, checked before allocation.
    pub length: u64,
}
/// Recovered metadata and source locations; no decoded pixels.
pub struct Board {
    /// All changes were replayed through the command layer.
    pub scene: Scene,
    /// Content-addressed original data, deduplicated per board.
    pub blobs: HashMap<BlobId, Blob>,
    /// True when bytes after the last committed footer were ignored.
    pub recovered: bool,
}
/// Starts a new stream; callers must use create-new storage, never an existing file.
pub fn initialize(writer: &mut impl Write) -> Result<(), Error> {
    writer.write_all(HEADER)?;
    Ok(())
}
/// Appends a record and its commit footer without changing existing bytes.
/// Durability is the caller's responsibility: sync before acknowledging a command.
pub fn append(
    writer: &mut impl Write,
    offset: u64,
    kind: u8,
    payload: &[u8],
) -> Result<u64, Error> {
    if payload.len() as u64 > MAX_RECORD || offset < HEADER.len() as u64 || !matches!(kind, 1..=3) {
        return Err(Error::Budget);
    }
    let mut footer = [0u8; FOOTER_SIZE as usize];
    footer[..8].copy_from_slice(FOOTER);
    footer[8..16].copy_from_slice(&offset.to_le_bytes());
    footer[16..24].copy_from_slice(&(payload.len() as u64).to_le_bytes());
    footer[24] = kind;
    footer[32..64].copy_from_slice(blake3::hash(payload).as_bytes());
    let checksum = blake3::hash(&footer[..64]);
    footer[64..].copy_from_slice(checksum.as_bytes());
    writer.write_all(payload)?;
    writer.write_all(&footer)?;
    Ok(offset + payload.len() as u64 + FOOTER_SIZE)
}
/// Encodes a command without relying on Rust enum layout or process-local keys.
pub fn encode(command: &Command) -> Vec<u8> {
    let mut bytes = Vec::new();
    match command {
        Command::Insert(item) => match &item.content {
            Content::Image(blob) => {
                bytes.push(11);
                bytes.extend_from_slice(item.id.as_bytes());
                bytes.extend_from_slice(blob.as_bytes());
                rect_bytes(&mut bytes, item.rect);
                transform_bytes(&mut bytes, item.transform);
                bytes.extend_from_slice(&(item.name.len() as u16).to_le_bytes());
                bytes.extend_from_slice(item.name.as_bytes());
                bytes.extend_from_slice(item.z.as_bytes());
            }
            Content::Text(text) => {
                bytes.push(12);
                bytes.extend_from_slice(item.id.as_bytes());
                bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
                bytes.extend_from_slice(text.as_bytes());
                rect_bytes(&mut bytes, item.rect);
                transform_bytes(&mut bytes, item.transform);
                bytes.extend_from_slice(&(item.name.len() as u16).to_le_bytes());
                bytes.extend_from_slice(item.name.as_bytes());
                bytes.extend_from_slice(item.z.as_bytes());
            }
        },
        Command::Remove(id) => {
            bytes.push(2);
            bytes.extend_from_slice(id.as_bytes());
        }
        Command::SetRect { id, rect } => {
            bytes.push(3);
            bytes.extend_from_slice(id.as_bytes());
            rect_bytes(&mut bytes, *rect);
        }
        Command::SetTransform { id, transform } => {
            bytes.push(13);
            bytes.extend_from_slice(id.as_bytes());
            transform_bytes(&mut bytes, *transform);
        }
        Command::SetText { id, text } => {
            bytes.push(7);
            bytes.extend_from_slice(id.as_bytes());
            bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
        }
    }
    bytes
}
/// Encodes an atomic checkpoint; the footer commits the entire replacement scene.
pub fn snapshot(scene: &Scene) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    for item in scene.items() {
        let command = encode(&Command::Insert(item.clone()));
        if bytes.len() + command.len() + 4 > 64 * 1024 * 1024 {
            return Err(Error::Budget);
        }
        bytes.extend_from_slice(&(command.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&command);
    }
    Ok(bytes)
}
fn restore(bytes: &[u8], blobs: &HashMap<BlobId, Blob>) -> Result<Scene, Error> {
    let mut scene = Scene::default();
    let mut rest = bytes;
    while !rest.is_empty() {
        let size = u32::from_le_bytes(
            rest.get(..4)
                .ok_or(Error::Invalid)?
                .try_into()
                .map_err(|_| Error::Invalid)?,
        ) as usize;
        rest = &rest[4..];
        let command = decode(rest.get(..size).ok_or(Error::Invalid)?)?;
        match &command {
            // A checkpoint may only insert items; image inserts must reference a committed blob.
            Command::Insert(item) if item.blob().is_none_or(|blob| blobs.contains_key(&blob)) => {}
            _ => return Err(Error::Invalid),
        }
        scene.apply(command)?;
        rest = &rest[size..];
    }
    Ok(scene)
}
fn rect_bytes(bytes: &mut Vec<u8>, r: Rect) {
    for v in [r.x, r.y, r.width, r.height] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
}
fn transform_bytes(bytes: &mut Vec<u8>, t: Transform) {
    bytes.extend_from_slice(&t.rotation.to_le_bytes());
    bytes.push(t.flip[0] as u8 | (t.flip[1] as u8) << 1);
    for v in t.content_offset {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes.extend_from_slice(&t.content_scale.to_le_bytes());
    bytes.extend_from_slice(&t.content_rotation.to_le_bytes());
    bytes.extend_from_slice(&t.opacity.to_le_bytes());
    bytes.push(t.grayscale as u8);
    bytes.push(t.sampling as u8);
    bytes.extend_from_slice(&t.note_font_size.to_le_bytes());
    bytes.push(t.note_wrap as u8);
}
fn decode(bytes: &[u8]) -> Result<Command, Error> {
    let id =
        Uuid::from_slice(bytes.get(1..17).ok_or(Error::Invalid)?).map_err(|_| Error::Invalid)?;
    let float = |start: usize, index: usize| -> Result<f32, Error> {
        Ok(f32::from_le_bytes(
            bytes
                .get(start + index * 4..start + index * 4 + 4)
                .ok_or(Error::Invalid)?
                .try_into()
                .map_err(|_| Error::Invalid)?,
        ))
    };
    let rect = || -> Result<Rect, Error> {
        let start = if matches!(bytes[0], 1 | 4 | 8 | 11) {
            49
        } else {
            17
        };
        Ok(Rect {
            x: float(start, 0)?,
            y: float(start, 1)?,
            width: float(start, 2)?,
            height: float(start, 3)?,
        })
    };
    let transform = |start: usize| -> Result<Transform, Error> {
        let flags = *bytes.get(start + 4).ok_or(Error::Invalid)?;
        Ok(Transform {
            rotation: float(start, 0)?,
            opacity: if matches!(bytes[0], 8..=13) {
                float(start + 21, 0)?
            } else {
                1.0
            },
            grayscale: if matches!(bytes[0], 8..=13) {
                match bytes.get(start + 25) {
                    Some(0) => false,
                    Some(1) => true,
                    _ => return Err(Error::Invalid),
                }
            } else {
                false
            },
            sampling: if matches!(bytes[0], 11..=13) {
                moth_scene::Sampling::from_code(*bytes.get(start + 26).ok_or(Error::Invalid)?)
                    .ok_or(Error::Invalid)?
            } else {
                moth_scene::Sampling::Default
            },
            note_font_size: if matches!(bytes[0], 11..=13) {
                float(start + 27, 0)?
            } else {
                0.
            },
            note_wrap: if matches!(bytes[0], 11..=13) {
                match bytes.get(start + 31) {
                    Some(0) => false,
                    Some(1) => true,
                    _ => return Err(Error::Invalid),
                }
            } else {
                false
            },
            flip: [flags & 1 != 0, flags & 2 != 0],
            content_offset: [float(start + 5, 0)?, float(start + 5, 1)?],
            content_scale: float(start + 13, 0)?,
            content_rotation: float(start + 17, 0)?,
        })
    };
    let name_index = |start: usize| -> Result<(String, usize), Error> {
        let len = u16::from_le_bytes(
            bytes
                .get(start..start + 2)
                .ok_or(Error::Invalid)?
                .try_into()
                .map_err(|_| Error::Invalid)?,
        ) as usize;
        if len > 1024 {
            return Err(Error::Budget);
        }
        let end = start + 2 + len;
        let name = std::str::from_utf8(bytes.get(start + 2..end).ok_or(Error::Invalid)?)
            .map_err(|_| Error::Invalid)?
            .to_owned();
        Ok((name, end))
    };
    match bytes[0] {
        // Schema-1 insert: no transform fields, so content starts at identity.
        1 if bytes.len() > 65 && bytes.len() <= 4161 => Ok(Command::Insert(Item {
            id,
            name: String::new(),
            content: Content::Image(BlobId::from_bytes(
                bytes[17..49].try_into().map_err(|_| Error::Invalid)?,
            )),
            rect: rect()?,
            transform: Transform::default(),
            z: fractional_index::FractionalIndex::from_bytes(bytes[65..].to_vec())
                .map_err(|_| Error::Invalid)?,
        })),
        4 | 8 | 11
            if bytes.len() > INSERT_V2_FIXED + if bytes[0] >= 8 { 5 } else { 0 }
                && bytes.len() as u64 <= MAX_COMMAND =>
        {
            let (name, index) = if bytes[0] >= 8 {
                name_index(if bytes[0] == 11 { 97 } else { 91 })?
            } else {
                (String::new(), 86)
            };
            Ok(Command::Insert(Item {
                id,
                name,
                content: Content::Image(BlobId::from_bytes(
                    bytes[17..49].try_into().map_err(|_| Error::Invalid)?,
                )),
                rect: rect()?,
                transform: transform(65)?,
                z: fractional_index::FractionalIndex::from_bytes(bytes[index..].to_vec())
                    .map_err(|_| Error::Invalid)?,
            }))
        }
        6 | 9 | 12
            if bytes.len()
                > INSERT_TEXT_FIXED
                    - if bytes[0] == 6 {
                        11
                    } else if bytes[0] == 9 {
                        6
                    } else {
                        0
                    }
                && bytes.len() as u64 <= MAX_TEXT_COMMAND =>
        {
            let text_len =
                u32::from_le_bytes(bytes[17..21].try_into().map_err(|_| Error::Invalid)?) as usize;
            if text_len > moth_scene::MAX_NOTE_BYTES {
                return Err(Error::Invalid);
            }
            let text_end = 21 + text_len;
            let text = std::str::from_utf8(bytes.get(21..text_end).ok_or(Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            let rect_start = text_end;
            let rect = Rect {
                x: float(rect_start, 0)?,
                y: float(rect_start, 1)?,
                width: float(rect_start, 2)?,
                height: float(rect_start, 3)?,
            };
            let transform_start = rect_start + 16;
            let (name, index) = if bytes[0] >= 9 {
                name_index(transform_start + if bytes[0] == 12 { 32 } else { 26 })?
            } else {
                (String::new(), transform_start + 21)
            };
            Ok(Command::Insert(Item {
                id,
                name,
                content: Content::Text(text.to_owned()),
                rect,
                transform: transform(transform_start)?,
                z: fractional_index::FractionalIndex::from_bytes(bytes[index..].to_vec())
                    .map_err(|_| Error::Invalid)?,
            }))
        }
        2 if bytes.len() == 17 => Ok(Command::Remove(id)),
        3 if bytes.len() == 33 => Ok(Command::SetRect { id, rect: rect()? }),
        5 | 10 | 13
            if bytes.len()
                == SET_TRANSFORM_LEN
                    + match bytes[0] {
                        13 => 11,
                        10 => 5,
                        _ => 0,
                    } =>
        {
            Ok(Command::SetTransform {
                id,
                transform: transform(17)?,
            })
        }
        7 if bytes.len() > 21 && bytes.len() as u64 <= MAX_TEXT_COMMAND => {
            let text_len =
                u32::from_le_bytes(bytes[17..21].try_into().map_err(|_| Error::Invalid)?) as usize;
            if text_len > moth_scene::MAX_NOTE_BYTES {
                return Err(Error::Invalid);
            }
            let text = std::str::from_utf8(bytes.get(21..21 + text_len).ok_or(Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            Ok(Command::SetText {
                id,
                text: text.to_owned(),
            })
        }
        _ => Err(Error::Invalid),
    }
}
/// Reads adjacent footers directly, retaining bounded scanning for damaged streams.
/// Torn tails and abandoned writes never prevent recovering later valid appends.
pub fn read(reader: &mut dyn ReadSeek) -> Result<Board, Error> {
    reader.seek(SeekFrom::Start(0))?;
    let mut header = [0; 16];
    reader.read_exact(&mut header)?;
    if &header != HEADER
        && &header != HEADER_V4
        && &header != HEADER_V3
        && &header != HEADER_V2
        && &header != HEADER_V1
    {
        return Err(Error::Invalid);
    }
    let length = reader.seek(SeekFrom::End(0))?;
    let mut footers = footer_chain(reader, length)?.map(Vec::into_iter);
    let mut board = Board {
        scene: Scene::default(),
        blobs: HashMap::new(),
        recovered: false,
    };
    let mut position = 16u64;
    let mut committed = 16u64;
    let mut buffer = vec![0; 64 * 1024 + FOOTER_SIZE as usize];
    'records: while position + FOOTER_SIZE <= length {
        let indexed = footers.as_mut().and_then(Iterator::next);
        if let Some(offset) = indexed {
            position = offset;
        } else if footers.is_some() {
            break;
        }
        reader.seek(SeekFrom::Start(position))?;
        let count = if indexed.is_some() {
            FOOTER_SIZE as usize
        } else {
            ((length - position).min(buffer.len() as u64)) as usize
        };
        reader.read_exact(&mut buffer[..count])?;
        let mut advance = count.saturating_sub(FOOTER_SIZE as usize - 1);
        for i in 0..advance {
            let f = &buffer[i..i + FOOTER_SIZE as usize];
            if &f[..8] != FOOTER || blake3::hash(&f[..64]).as_bytes() != &f[64..96] {
                if indexed.is_some() {
                    footers = None;
                    position = committed;
                    continue 'records;
                }
                continue;
            }
            let start = u64::from_le_bytes(f[8..16].try_into().map_err(|_| Error::Invalid)?);
            let size = u64::from_le_bytes(f[16..24].try_into().map_err(|_| Error::Invalid)?);
            let end = position + i as u64;
            if start < committed || size > MAX_RECORD || start.checked_add(size) != Some(end) {
                continue;
            }
            reader.seek(SeekFrom::Start(start))?;
            let mut hash = blake3::Hasher::new();
            let mut remaining = size;
            let mut scratch = [0; 64 * 1024];
            while remaining > 0 {
                let n = remaining.min(scratch.len() as u64) as usize;
                reader.read_exact(&mut scratch[..n])?;
                hash.update(&scratch[..n]);
                remaining -= n as u64;
            }
            if hash.finalize().as_bytes() != &f[32..64] {
                if indexed.is_some() {
                    // Corruption keeps the scanner's existing skip-and-recover semantics.
                    footers = None;
                    position = committed;
                    continue 'records;
                }
                continue;
            }
            match f[24] {
                1 => {
                    if board.blobs.len() >= 1_000_000 {
                        return Err(Error::Budget);
                    }
                    board.blobs.insert(
                        hash.finalize(),
                        Blob {
                            offset: start,
                            length: size,
                        },
                    );
                }
                2 => {
                    if size > MAX_TEXT_COMMAND || board.scene.len() >= 1_000_000 {
                        return Err(Error::Budget);
                    }
                    reader.seek(SeekFrom::Start(start))?;
                    let mut bytes = vec![0; size as usize];
                    reader.read_exact(&mut bytes)?;
                    let command = decode(&bytes)?;
                    if let Command::Insert(item) = &command
                        && item
                            .blob()
                            .is_some_and(|blob| !board.blobs.contains_key(&blob))
                    {
                        return Err(Error::Invalid);
                    }
                    board.scene.apply(command)?;
                }
                3 => {
                    if size > 64 * 1024 * 1024 {
                        return Err(Error::Budget);
                    }
                    reader.seek(SeekFrom::Start(start))?;
                    let mut bytes = vec![0; size as usize];
                    reader.read_exact(&mut bytes)?;
                    board.scene = restore(&bytes, &board.blobs)?;
                }
                _ => return Err(Error::Invalid),
            }
            board.recovered |= start != committed;
            committed = end + FOOTER_SIZE;
            advance = i + FOOTER_SIZE as usize;
            break;
        }
        position += advance as u64;
    }
    board.recovered |= committed != length;
    Ok(board)
}

// Healthy files link backwards through adjacent footers; torn/gapped files use the scanner.
fn footer_chain(reader: &mut dyn ReadSeek, mut end: u64) -> Result<Option<Vec<u64>>, Error> {
    let mut offsets = Vec::new();
    let mut footer = [0; FOOTER_SIZE as usize];
    while end != 16 {
        if end < 16 + FOOTER_SIZE || offsets.len() == 1_000_000 {
            return Ok(None);
        }
        let offset = end - FOOTER_SIZE;
        reader.seek(SeekFrom::Start(offset))?;
        reader.read_exact(&mut footer)?;
        if &footer[..8] != FOOTER || blake3::hash(&footer[..64]).as_bytes() != &footer[64..] {
            return Ok(None);
        }
        let start = u64::from_le_bytes(footer[8..16].try_into().map_err(|_| Error::Invalid)?);
        let size = u64::from_le_bytes(footer[16..24].try_into().map_err(|_| Error::Invalid)?);
        if start < 16 || size > MAX_RECORD || start.checked_add(size) != Some(offset) {
            return Ok(None);
        }
        offsets.push(offset);
        end = start;
    }
    offsets.reverse();
    Ok(Some(offsets))
}

/// Compacts live originals and one checkpoint through an atomic storage replacement.
/// Failure before replacement leaves the original stream intact.
pub fn compact(storage: &dyn crate::Storage, handle: crate::Handle) -> Result<(), Error> {
    let mut reader = storage.open(handle)?;
    let board = read(reader.as_mut())?;
    let temporary = storage.temporary(handle)?;
    let mut writer = storage.append(temporary)?;
    initialize(&mut writer)?;
    let mut offset = 16;
    let mut copied = std::collections::HashSet::new();
    for item in board.scene.items() {
        // Notes carry no original bytes; only image blobs are copied forward.
        let Some(blob_id) = item.blob() else {
            continue;
        };
        if !copied.insert(blob_id) {
            continue;
        }
        let blob = board.blobs.get(&blob_id).ok_or(Error::Invalid)?;
        reader.seek(SeekFrom::Start(blob.offset))?;
        let mut bytes = vec![0; blob.length as usize];
        reader.read_exact(&mut bytes)?;
        if blake3::hash(&bytes) != blob_id {
            return Err(Error::Invalid);
        }
        offset = append(&mut writer, offset, 1, &bytes)?;
    }
    append(&mut writer, offset, 3, &snapshot(&board.scene)?)?;
    writer.flush()?;
    drop(writer);
    drop(reader);
    storage.sync(temporary)?;
    storage.replace(temporary, handle)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::io::Cursor;
    #[test]
    fn healthy_records_are_hashed_without_scanning_their_payloads() {
        struct Count(Cursor<Vec<u8>>, usize);
        impl std::io::Read for Count {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                let count = std::io::Read::read(&mut self.0, bytes)?;
                self.1 += count;
                Ok(count)
            }
        }
        impl std::io::Seek for Count {
            fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
                std::io::Seek::seek(&mut self.0, from)
            }
        }
        let original = vec![42; 2 * 1024 * 1024];
        let (_, mut item) = base();
        item.content = Content::Image(blake3::hash(&original));
        let mut bytes = Vec::new();
        initialize(&mut bytes).unwrap();
        append(&mut bytes, 16, 1, &original).unwrap();
        let offset = bytes.len() as u64;
        append(
            &mut bytes,
            offset,
            2,
            &encode(&Command::Insert(item.clone())),
        )
        .unwrap();
        let mut reader = Count(Cursor::new(bytes), 0);
        assert_eq!(read(&mut reader).unwrap().scene.get(item.id), Some(&item));
        assert!(reader.1 < original.len() + 2048);
    }
    fn base() -> (Vec<u8>, Item) {
        let mut bytes = Vec::new();
        initialize(&mut bytes).unwrap();
        append(&mut bytes, 16, 1, b"original").unwrap();
        let item = Item::image(
            blake3::hash(b"original"),
            Rect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            None,
        );
        let offset = bytes.len() as u64;
        append(
            &mut bytes,
            offset,
            2,
            &encode(&Command::Insert(item.clone())),
        )
        .unwrap();
        (bytes, item)
    }
    #[test]
    fn every_truncation_retains_previous_checkpoint() {
        let (mut bytes, item) = base();
        let previous = bytes.len();
        append(
            &mut bytes,
            previous as u64,
            2,
            &encode(&Command::Remove(item.id)),
        )
        .unwrap();
        for cut in previous..bytes.len() {
            let board = read(&mut Cursor::new(&bytes[..cut])).unwrap();
            assert_eq!(board.scene.get(item.id), Some(&item));
        }
        assert!(read(&mut Cursor::new(&bytes)).unwrap().scene.is_empty());
    }
    #[test]
    fn checkpoint_is_atomic_at_every_truncation() {
        let (mut bytes, item) = base();
        let start = bytes.len();
        append(
            &mut bytes,
            start as u64,
            3,
            &snapshot(&Scene::default()).unwrap(),
        )
        .unwrap();
        for cut in start..bytes.len() {
            assert_eq!(
                read(&mut Cursor::new(&bytes[..cut]))
                    .unwrap()
                    .scene
                    .get(item.id),
                Some(&item)
            );
        }
        assert!(read(&mut Cursor::new(bytes)).unwrap().scene.is_empty());
    }
    #[test]
    fn unsupported_schema_is_rejected() {
        let (mut bytes, _) = base();
        bytes[4] = 6;
        assert!(matches!(read(&mut Cursor::new(bytes)), Err(Error::Invalid)));
    }
    #[test]
    fn schema_five_fixture_keeps_sampling_and_wrapped_note_metrics() {
        let bytes = include_bytes!("../../../corpus/moth/v5.moth");
        assert_eq!(bytes[4], 5);
        let board = read(&mut Cursor::new(bytes.as_slice())).unwrap();
        assert!(board.scene.items().any(|item| item.blob().is_some()
            && item.transform.sampling == moth_scene::Sampling::Nearest));
        assert!(board.scene.items().any(|item| item.note().is_some()
            && item.transform.note_wrap
            && item.transform.note_font_size == 24.));
        let restored = restore(&snapshot(&board.scene).unwrap(), &board.blobs).unwrap();
        for item in board.scene.items() {
            assert_eq!(restored.get(item.id), Some(item));
        }
    }
    #[test]
    fn schema_four_migrates_without_losing_appearance_or_text() {
        let bytes = include_bytes!("../../../corpus/moth/v4.moth");
        assert_eq!(bytes[4], 4);
        let old = read(&mut Cursor::new(bytes.as_slice())).unwrap();
        let migrated = restore(&snapshot(&old.scene).unwrap(), &old.blobs).unwrap();
        for item in old.scene.items() {
            assert_eq!(item.transform.sampling, moth_scene::Sampling::Default);
            assert_eq!(item.transform.note_font_size, 0.);
            assert!(!item.transform.note_wrap);
            assert_eq!(migrated.get(item.id), Some(item));
        }
    }
    #[test]
    fn invalid_schema_five_sampling_and_note_fields_are_rejected() {
        let (_, item) = base();
        let transform = Transform {
            note_font_size: 24.,
            note_wrap: true,
            ..Default::default()
        };
        let valid = encode(&Command::SetTransform {
            id: item.id,
            transform,
        });
        assert!(decode(&valid).is_ok());
        for (offset, value) in [(17 + 26, 4), (17 + 31, 2)] {
            let mut invalid = valid.clone();
            invalid[offset] = value;
            assert!(decode(&invalid).is_err());
        }
        for cut in 0..valid.len() {
            assert!(decode(&valid[..cut]).is_err());
        }
    }
    #[test]
    fn schema_three_migrates_appearance_and_names() {
        let bytes = include_bytes!("../../../corpus/moth/v3.moth");
        assert_eq!(bytes[4], 3);
        let old = read(&mut Cursor::new(bytes.as_slice())).unwrap();
        for item in old.scene.items() {
            assert_eq!(item.transform.opacity, 1.);
            assert!(!item.transform.grayscale);
            assert!(item.name.is_empty());
        }
        let migrated = restore(&snapshot(&old.scene).unwrap(), &old.blobs).unwrap();
        for item in old.scene.items() {
            assert_eq!(migrated.get(item.id), Some(item));
        }
    }
    #[test]
    fn schema_one_board_still_loads() {
        // Frozen schema-1 fixture: regenerating it with the current writer would erase the
        // migration case this test exists for.
        const V1: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/moth/v1.moth"
        ));
        assert_eq!(V1[4], 1, "corpus/moth/v1.moth must stay schema 1");
        let board = read(&mut Cursor::new(V1)).unwrap();
        assert_eq!(board.scene.len(), 1);
        let item = board.scene.items().next().unwrap();
        assert_eq!(item.transform, Transform::default());
        assert!(board.blobs.contains_key(&item.blob().unwrap()));
    }
    #[test]
    fn transform_survives_records_checkpoints_and_truncation() {
        let (mut bytes, item) = base();
        let transform = Transform {
            rotation: 0.75,
            flip: [true, false],
            content_offset: [-0.25, 1.5],
            content_scale: 2.5,
            content_rotation: -0.5,
            sampling: moth_scene::Sampling::Nearest,
            note_font_size: 24.,
            note_wrap: true,
            ..Transform::default()
        };
        let mut scene = read(&mut Cursor::new(&bytes)).unwrap().scene;
        scene
            .apply(Command::SetTransform {
                id: item.id,
                transform,
            })
            .unwrap();
        let start = bytes.len();
        append(
            &mut bytes,
            start as u64,
            2,
            &encode(&Command::SetTransform {
                id: item.id,
                transform,
            }),
        )
        .unwrap();
        assert_eq!(
            read(&mut Cursor::new(&bytes))
                .unwrap()
                .scene
                .get(item.id)
                .unwrap()
                .transform,
            transform
        );
        let mut restored = restore(
            &snapshot(&scene).unwrap(),
            &read(&mut Cursor::new(&bytes)).unwrap().blobs,
        )
        .unwrap();
        assert_eq!(restored.get(item.id).unwrap().transform, transform);
        restored
            .apply(Command::SetTransform {
                id: item.id,
                transform: Transform::default(),
            })
            .unwrap();
        assert_eq!(
            restored.get(item.id).unwrap().transform,
            Transform::default()
        );
        // A torn transform record never becomes a committed change.
        for cut in start..bytes.len() {
            assert_eq!(
                read(&mut Cursor::new(&bytes[..cut]))
                    .unwrap()
                    .scene
                    .get(item.id)
                    .unwrap()
                    .transform,
                Transform::default()
            );
        }
    }
    #[test]
    fn append_after_torn_record_and_corrupt_record() {
        let (mut bytes, item) = base();
        bytes.extend_from_slice(b"abandoned write");
        let offset = bytes.len() as u64;
        append(&mut bytes, offset, 2, &encode(&Command::Remove(item.id))).unwrap();
        assert!(read(&mut Cursor::new(&bytes)).unwrap().scene.is_empty());
        bytes[offset as usize] ^= 1;
        assert_eq!(
            read(&mut Cursor::new(bytes)).unwrap().scene.get(item.id),
            Some(&item)
        );
    }
    #[test]
    fn note_survives_insert_edit_checkpoint_and_truncation() {
        let mut bytes = Vec::new();
        initialize(&mut bytes).unwrap();
        let rect = Rect {
            x: 5.,
            y: 6.,
            width: 20.,
            height: 10.,
        };
        let note = Item::text("hello", rect, None);
        let id = note.id;
        let offset = bytes.len() as u64;
        append(&mut bytes, offset, 2, &encode(&Command::Insert(note))).unwrap();
        let start = bytes.len();
        append(
            &mut bytes,
            start as u64,
            2,
            &encode(&Command::SetText {
                id,
                text: "world".into(),
            }),
        )
        .unwrap();
        // A torn SetText never becomes a committed change.
        for cut in start..bytes.len() {
            let board = read(&mut Cursor::new(&bytes[..cut])).unwrap();
            assert_eq!(board.scene.get(id).unwrap().note(), Some("hello"));
        }
        let board = read(&mut Cursor::new(&bytes)).unwrap();
        assert_eq!(board.scene.get(id).unwrap().note(), Some("world"));
        assert!(board.blobs.is_empty());
        // A checkpoint restores notes without any blob, and undo removes cleanly.
        let restored = restore(&snapshot(&board.scene).unwrap(), &board.blobs).unwrap();
        assert_eq!(restored.get(id), board.scene.get(id));
        let mut scene = restored;
        let inverse = scene.apply(Command::Remove(id)).unwrap();
        assert!(scene.get(id).is_none());
        scene.apply(inverse).unwrap();
        assert_eq!(scene.get(id).unwrap().note(), Some("world"));
    }
    proptest! {
        #[test]
        fn scene_roundtrip(x in -10000f32..10000., y in -10000f32..10000., rotation in -10f32..10., scale in 0.01f32..100., dx in -5f32..5., dy in -5f32..5., mirror in any::<bool>(), spin in -10f32..10., opacity in 0f32..=1., grayscale in any::<bool>(), sampling in 0u8..4, note_font_size in 1f32..200., note_wrap in any::<bool>(), name in "[a-zA-Z0-9_]{0,40}[.]png") {
            let (mut bytes,mut item)=base();
            item.rect.x=x; item.rect.y=y; item.name=name;
            item.transform=Transform{opacity,grayscale,sampling:moth_scene::Sampling::from_code(sampling).unwrap(),note_font_size,note_wrap,rotation,flip:[mirror,!mirror],content_offset:[dx,dy],content_scale:scale,content_rotation:spin};
            let offset=bytes.len() as u64;
            append(&mut bytes,offset,2,&encode(&Command::Remove(item.id))).unwrap();
            let offset=bytes.len() as u64;
            append(&mut bytes,offset,2,&encode(&Command::Insert(item.clone()))).unwrap();
            let offset=bytes.len() as u64;
            append(&mut bytes,offset,2,&encode(&Command::SetRect{id:item.id,rect:item.rect})).unwrap();
            let offset=bytes.len() as u64;
            append(&mut bytes,offset,2,&encode(&Command::SetTransform{id:item.id,transform:item.transform})).unwrap();
            let board=read(&mut Cursor::new(bytes)).unwrap();
            prop_assert_eq!(board.scene.get(item.id),Some(&item));
            prop_assert_eq!(board.scene.top(),Some(&item.z));
            let checkpoint=snapshot(&board.scene).unwrap();
            let mut restored=restore(&checkpoint,&board.blobs).unwrap();
            prop_assert_eq!(restored.get(item.id),Some(&item));
            prop_assert_eq!(restored.top(),Some(&item.z));
            restored.apply(Command::Remove(item.id)).unwrap();
            prop_assert!(restored.top().is_none());
        }
        #[test]
        fn arbitrary_input_never_panics(bytes in proptest::collection::vec(any::<u8>(),0..8192)) {
            let _=read(&mut Cursor::new(bytes));
        }
    }
}
