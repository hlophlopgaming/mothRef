// SPDX-License-Identifier: GPL-3.0-or-later
//! Version-one append-only records. A checksummed footer commits one record.
use crate::ReadSeek;
use moth_scene::{BlobId, Command, Item, Rect, Scene, Transform, Uuid};
use std::{
    collections::HashMap,
    io::{self, SeekFrom, Write},
};

const HEADER: &[u8; 16] = b"MOTH\x02\0\0\0\0\0\0\0\0\0\0\0";
/// Schema-1 boards predate content transforms; the reader still accepts them.
const HEADER_V1: &[u8; 16] = b"MOTH\x01\0\0\0\0\0\0\0\0\0\0\0";
const FOOTER: &[u8; 8] = b"MOTHEND1";
const FOOTER_SIZE: u64 = 96;
/// Bounds allocations even for malicious input lengths.
pub const MAX_RECORD: u64 = 256 * 1024 * 1024;
/// Longest accepted command payload: the largest fractional index plus its fixed fields.
const MAX_COMMAND: u64 = 4182;
/// Bytes of fixed command fields after the tag, before a schema-2 insert's fractional index.
const INSERT_V2_FIXED: usize = 86;
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
        Command::Insert(item) => {
            bytes.push(4);
            bytes.extend_from_slice(item.id.as_bytes());
            bytes.extend_from_slice(item.blob.as_bytes());
            rect_bytes(&mut bytes, item.rect);
            transform_bytes(&mut bytes, item.transform);
            bytes.extend_from_slice(item.z.as_bytes());
        }
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
            bytes.push(5);
            bytes.extend_from_slice(id.as_bytes());
            transform_bytes(&mut bytes, *transform);
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
            Command::Insert(item) if blobs.contains_key(&item.blob) => {}
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
        let start = if matches!(bytes[0], 1 | 4) { 49 } else { 17 };
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
            flip: [flags & 1 != 0, flags & 2 != 0],
            content_offset: [float(start + 5, 0)?, float(start + 5, 1)?],
            content_scale: float(start + 13, 0)?,
            content_rotation: float(start + 17, 0)?,
        })
    };
    match bytes[0] {
        // Schema-1 insert: no transform fields, so content starts at identity.
        1 if bytes.len() > 65 && bytes.len() <= 4161 => Ok(Command::Insert(Item {
            id,
            blob: BlobId::from_bytes(bytes[17..49].try_into().map_err(|_| Error::Invalid)?),
            rect: rect()?,
            transform: Transform::default(),
            z: fractional_index::FractionalIndex::from_bytes(bytes[65..].to_vec())
                .map_err(|_| Error::Invalid)?,
        })),
        4 if bytes.len() > INSERT_V2_FIXED && bytes.len() as u64 <= MAX_COMMAND => {
            Ok(Command::Insert(Item {
                id,
                blob: BlobId::from_bytes(bytes[17..49].try_into().map_err(|_| Error::Invalid)?),
                rect: rect()?,
                transform: transform(65)?,
                z: fractional_index::FractionalIndex::from_bytes(bytes[INSERT_V2_FIXED..].to_vec())
                    .map_err(|_| Error::Invalid)?,
            }))
        }
        2 if bytes.len() == 17 => Ok(Command::Remove(id)),
        3 if bytes.len() == 33 => Ok(Command::SetRect { id, rect: rect()? }),
        5 if bytes.len() == SET_TRANSFORM_LEN => Ok(Command::SetTransform {
            id,
            transform: transform(17)?,
        }),
        _ => Err(Error::Invalid),
    }
}
/// Scans bounded buffers for committed records, skipping torn tails and abandoned writes.
/// A valid later append can therefore follow a failed write without truncating the file.
pub fn read(reader: &mut dyn ReadSeek) -> Result<Board, Error> {
    reader.seek(SeekFrom::Start(0))?;
    let mut header = [0; 16];
    reader.read_exact(&mut header)?;
    if &header != HEADER && &header != HEADER_V1 {
        return Err(Error::Invalid);
    }
    let length = reader.seek(SeekFrom::End(0))?;
    let mut board = Board {
        scene: Scene::default(),
        blobs: HashMap::new(),
        recovered: false,
    };
    let mut position = 16u64;
    let mut committed = 16u64;
    let mut buffer = vec![0; 64 * 1024 + FOOTER_SIZE as usize];
    while position + FOOTER_SIZE <= length {
        reader.seek(SeekFrom::Start(position))?;
        let count = ((length - position).min(buffer.len() as u64)) as usize;
        reader.read_exact(&mut buffer[..count])?;
        let mut advance = count.saturating_sub(FOOTER_SIZE as usize - 1);
        for i in 0..advance {
            let f = &buffer[i..i + FOOTER_SIZE as usize];
            if &f[..8] != FOOTER || blake3::hash(&f[..64]).as_bytes() != &f[64..96] {
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
                    if size > MAX_COMMAND || board.scene.len() >= 1_000_000 {
                        return Err(Error::Budget);
                    }
                    reader.seek(SeekFrom::Start(start))?;
                    let mut bytes = vec![0; size as usize];
                    reader.read_exact(&mut bytes)?;
                    let command = decode(&bytes)?;
                    if let Command::Insert(item) = &command
                        && !board.blobs.contains_key(&item.blob)
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
        if !copied.insert(item.blob) {
            continue;
        }
        let blob = board.blobs.get(&item.blob).ok_or(Error::Invalid)?;
        reader.seek(SeekFrom::Start(blob.offset))?;
        let mut bytes = vec![0; blob.length as usize];
        reader.read_exact(&mut bytes)?;
        if blake3::hash(&bytes) != item.blob {
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
        bytes[4] = 3;
        assert!(matches!(read(&mut Cursor::new(bytes)), Err(Error::Invalid)));
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
        assert!(board.blobs.contains_key(&item.blob));
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
    proptest! {
        #[test]
        fn scene_roundtrip(x in -10000f32..10000., y in -10000f32..10000., rotation in -10f32..10., scale in 0.01f32..100., dx in -5f32..5., dy in -5f32..5., mirror in any::<bool>(), spin in -10f32..10.) {
            let (mut bytes,mut item)=base();
            item.rect.x=x; item.rect.y=y;
            item.transform=Transform{rotation,flip:[mirror,!mirror],content_offset:[dx,dy],content_scale:scale,content_rotation:spin};
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
