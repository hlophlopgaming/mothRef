// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded stream adapter; never materializes a SQLite database on disk.
use crate::{
    blobs, legacy, model,
    sqlite::{Database, ReadSeek, Value},
};
use md5::{Digest, Md5};
use std::{
    collections::HashMap,
    io::{self, Read, Seek, SeekFrom},
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Maximum encoded board size; payloads and metadata have separate RAM budgets.
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// Import limits and corrupt inputs are recoverable errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Byte source or callback failed.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// The embedded database is corrupt or unsupported.
    #[error(transparent)]
    Sqlite(#[from] crate::sqlite::Error),
    /// Admission errors are distinct from a corrupt file.
    #[error("{0}")]
    Budget(&'static str),
    /// Invalid input includes upstream codec panics, caught at the trust boundary.
    #[error("invalid or unsupported PureRef file (or import budget exceeded)")]
    Invalid,
}

/// Payloads are borrowed for one callback, so callers can stage originals immediately.
pub enum Entry<'a> {
    /// One placement of an embedded or linked image.
    Image(&'a model::Item, &'a model::ItemImage, &'a model::Image),
    /// One text note.
    Note(&'a model::Item, &'a model::Note),
    /// A subtype the caller cannot yet represent.
    Unsupported,
    /// Legacy boards emit one original at a time after resolving placement metadata.
    Legacy(&'a legacy::LegacyScene),
}

/// Reads 1.x/2.x without paths; malformed codecs never unwind into the worker.
/// Files are capped at 4 GiB, metadata at 8 MiB / 8192 items and payloads at 32 MiB.
pub fn read<S: ReadSeek>(
    mut source: S,
    emit: &mut impl FnMut(Entry<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    catch_unwind(AssertUnwindSafe(|| {
        let length = source.seek(SeekFrom::End(0))?;
        if length > MAX_FILE_BYTES {
            return Err(Error::Budget("PureRef board exceeds 4 GiB file limit"));
        }
        source.seek(SeekFrom::Start(0))?;
        let mut prefix = [0u8; 108];
        source.read_exact(&mut prefix)?;
        // Reject implausible version strings before selecting a reader.
        if u32::from_be_bytes(prefix[..4].try_into().map_err(|_| Error::Invalid)?) > 64 {
            return Err(Error::Invalid);
        }
        if prefix.get(5) == Some(&b'1') {
            return legacy::read(source, length, emit);
        }
        if &prefix[..10] != b"\0\0\0\x06\0\x32\0\x2e\0\x31"
            && &prefix[..10] != b"\0\0\0\x06\0\x32\0\x2e\0\x30"
        {
            return Err(Error::Invalid);
        }
        if prefix[22..26] != 10u32.to_be_bytes() || prefix[36..40] != 64u32.to_be_bytes() {
            return Err(Error::Invalid);
        }
        let db_len = u64::from_be_bytes(prefix[14..22].try_into().map_err(|_| Error::Invalid)?);
        let chunk = 108
            + u32::from_be_bytes(prefix[104..108].try_into().map_err(|_| Error::Invalid)?) as u64;
        if db_len < chunk || db_len.checked_add(chunk) != Some(length) {
            return Err(Error::Invalid);
        }
        verify_checksum(&mut source, length, 104, &prefix[40..104])?;
        read_database(
            Database::open(Splice {
                source,
                db_len,
                chunk,
                position: 0,
            })?,
            emit,
        )
    }))
    .map_err(|_| Error::Invalid)?
}

pub(crate) fn verify_checksum(
    source: &mut impl ReadSeek,
    length: u64,
    offset: u64,
    stored: &[u8],
) -> Result<(), Error> {
    source.seek(SeekFrom::Start(offset))?;
    let mut remaining = length.checked_sub(offset).ok_or(Error::Invalid)?;
    let mut digest = Md5::new();
    let mut buffer = [0u8; 65536];
    while remaining > 0 {
        let take = remaining.min(buffer.len() as u64) as usize;
        let n = source.read(&mut buffer[..take])?;
        if n == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
        }
        digest.update(&buffer[..n]);
        remaining -= n as u64;
    }
    let expected: String = digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    require!(stored.len() == 64);
    if !stored
        .as_chunks::<2>()
        .0
        .iter()
        .zip(expected.bytes())
        .all(|(p, b)| *p == [0, b])
    {
        return Err(Error::Invalid);
    }
    Ok(())
}

struct Splice<S> {
    source: S,
    db_len: u64,
    chunk: u64,
    position: u64,
}
impl<S: ReadSeek> Read for Splice<S> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.db_len {
            return Ok(0);
        }
        let (offset, remaining) = if self.position < self.chunk {
            (self.db_len + self.position, self.chunk - self.position)
        } else {
            (self.position, self.db_len - self.position)
        };
        let n = bytes
            .len()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        self.source.seek(SeekFrom::Start(offset))?;
        let n = self.source.read(&mut bytes[..n])?;
        self.position += n as u64;
        Ok(n)
    }
}
impl<S: ReadSeek> Seek for Splice<S> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let next = match position {
            SeekFrom::Start(v) => i128::from(v),
            SeekFrom::End(v) => i128::from(self.db_len) + i128::from(v),
            SeekFrom::Current(v) => i128::from(self.position) + i128::from(v),
        };
        self.position =
            u64::try_from(next).map_err(|_| io::Error::other("invalid database seek"))?;
        Ok(self.position)
    }
}
fn blob(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::Text(s) => Some(blobs::stored_text_to_bytes(s)),
        Value::Blob(b) => Some(b.clone()),
        _ => None,
    }
}
fn string(value: &Value) -> Option<String> {
    value.text().map(str::to_owned)
}
fn transform(value: &Value) -> Result<Option<blobs::QTransform>, Error> {
    blob(value).map(|b| blobs::decode_transform(&b)).transpose()
}
fn charge(row: &[Value], budget: &mut usize) -> Result<(), Error> {
    let size: usize = row
        .iter()
        .map(|v| match v {
            Value::Text(s) => s.len(),
            Value::Blob(b) => b.len(),
            _ => 32,
        })
        .sum();
    *budget = budget.checked_add(size).ok_or(Error::Invalid)?;
    if *budget > 8 * 1024 * 1024 {
        return Err(Error::Invalid);
    }
    Ok(())
}
fn table<S: ReadSeek>(
    db: &mut Database<S>,
    name: &str,
    expected: &[&str],
) -> Result<(u32, Vec<usize>), Error> {
    let root = db.root_page(name)?.ok_or(Error::Invalid)?;
    let columns = db.columns(name)?;
    if columns.len() != expected.len() {
        return Err(Error::Invalid);
    }
    let order = expected
        .iter()
        .map(|name| {
            columns
                .iter()
                .position(|column| column == name)
                .ok_or(Error::Invalid)
        })
        .collect::<Result<_, _>>()?;
    Ok((root, order))
}
fn ordered(mut row: Vec<Value>, order: &[usize]) -> Result<Vec<Value>, Error> {
    if row.len() != order.len() {
        return Err(Error::Invalid);
    }
    order
        .iter()
        .map(|&i| {
            row.get_mut(i)
                .map(|v| std::mem::replace(v, Value::Null))
                .ok_or(Error::Invalid)
        })
        .collect()
}
fn read_database<S: ReadSeek>(
    mut db: Database<S>,
    emit: &mut impl FnMut(Entry<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut budget = 0;
    let (root, order) = table(
        &mut db,
        "items",
        &[
            "comment",
            "opacity",
            "z",
            "locked",
            "parent",
            "sort_order",
            "transform",
            "name",
            "id",
        ],
    )?;
    let mut items = HashMap::new();
    db.walk_table(root, &mut |id, row| -> Result<(), Error> {
        let row = ordered(row, &order)?;
        charge(&row, &mut budget)?;
        if row.len() != 9 || items.len() >= 8192 {
            return Err(Error::Invalid);
        }
        items.insert(
            id,
            model::Item {
                id,
                comment: string(&row[0]),
                opacity: row[1].real(),
                z: row[2].real(),
                locked: row[3].integer().map(|v| v as i32),
                parent: row[4].integer().unwrap_or(-1),
                sort_order: blob(&row[5])
                    .map(|b| blobs::decode_big_rational(&b))
                    .transpose()?,
                transform: transform(&row[6])?,
                name: string(&row[7]),
            },
        );
        Ok(())
    })?;
    // Flatten parent transforms while all base rows are still available. Group
    // decoration remains unsupported, but grouped images keep their placement.
    let mut flattened = HashMap::with_capacity(items.len());
    for (&id, item) in &items {
        let mut world = item.transform.unwrap_or(blobs::QTransform::IDENTITY);
        let mut opacity = item.opacity.unwrap_or(1.);
        let mut parent = item.parent;
        let mut depth = 0;
        while parent != -1 {
            depth += 1;
            if depth > 64 || parent == id {
                return Err(Error::Invalid);
            }
            let ancestor = items.get(&parent).ok_or(Error::Invalid)?;
            world = compose(
                ancestor.transform.unwrap_or(blobs::QTransform::IDENTITY),
                world,
            );
            opacity *= ancestor.opacity.unwrap_or(1.);
            parent = ancestor.parent;
        }
        flattened.insert(id, (world, opacity));
    }
    for (id, item) in &mut items {
        let (world, opacity) = flattened.remove(id).ok_or(Error::Invalid)?;
        item.transform = Some(world);
        item.opacity = Some(opacity);
        item.parent = -1;
    }
    let (root, order) = table(
        &mut db,
        "items_images",
        &[
            "playback_speed",
            "playback_frame",
            "playback_state",
            "image_bounds",
            "image_transform",
            "flags",
            "image",
            "id",
        ],
    )?;
    let mut placements: HashMap<i64, Vec<model::ItemImage>> = HashMap::new();
    let mut placement_count = 0;
    db.walk_table(root, &mut |id, row| -> Result<(), Error> {
        let row = ordered(row, &order)?;
        charge(&row, &mut budget)?;
        placement_count += 1;
        if row.len() != 8 || placement_count > 8192 {
            return Err(Error::Invalid);
        }
        let placement = model::ItemImage {
            id,
            image: row[6].integer(),
            image_transform: transform(&row[4])?,
            image_bounds: blob(&row[3]).map(|b| blobs::decode_path(&b)).transpose()?,
            flags: row[5].integer().map(|v| v as i32),
            ..Default::default()
        };
        if let Some(image) = placement.image {
            placements.entry(image).or_default().push(placement);
        } else {
            emit(Entry::Unsupported)?;
            items.remove(&id);
        }
        Ok(())
    })?;
    let (root, order) = table(
        &mut db,
        "images",
        &[
            "width",
            "checksum",
            "format",
            "source",
            "height",
            "data",
            "source_type",
            "origin",
            "id",
        ],
    )?;
    db.walk_table(root, &mut |id, row| -> Result<(), Error> {
        let mut row = ordered(row, &order)?;
        if row.len() != 9 {
            return Err(Error::Invalid);
        }
        if let Some(placements) = placements.remove(&id) {
            let data = match std::mem::replace(&mut row[5], Value::Null) {
                Value::Blob(b) => Some(b),
                _ => None,
            };
            let image = model::Image {
                id,
                width: row[0].integer().and_then(|v| v.try_into().ok()),
                height: row[4].integer().and_then(|v| v.try_into().ok()),
                data,
                source: string(&row[3]),
                source_type: row[6].integer().map(|v| v as i32),
                ..Default::default()
            };
            for placement in placements {
                if let Some(item) = items.remove(&placement.id) {
                    emit(Entry::Image(&item, &placement, &image))?;
                } else {
                    return Err(Error::Invalid);
                }
            }
        }
        Ok(())
    })?;
    let (root, order) = table(
        &mut db,
        "items_notes",
        &[
            "fixed_size",
            "background_color",
            "text_color",
            "style",
            "text",
            "id",
        ],
    )?;
    db.walk_table(root, &mut |id, row| -> Result<(), Error> {
        let row = ordered(row, &order)?;
        charge(&row, &mut budget)?;
        if row.len() != 6 {
            return Err(Error::Invalid);
        }
        if let Some(item) = items.remove(&id) {
            emit(Entry::Note(
                &item,
                &model::Note {
                    id,
                    text: string(&row[4]),
                    fixed_size: string(&row[0]),
                    ..Default::default()
                },
            ))?;
        } else {
            return Err(Error::Invalid);
        }
        Ok(())
    })?;
    for _ in items {
        emit(Entry::Unsupported)?;
    }
    Ok(())
}

fn compose(p: blobs::QTransform, c: blobs::QTransform) -> blobs::QTransform {
    let a = p.values();
    let b = c.values();
    let mut result = [0.; 9];
    // QTransform stores row vectors: child * parent maps local to world.
    for row in 0..3 {
        for column in 0..3 {
            result[row * 3 + column] = (0..3).map(|k| b[row * 3 + k] * a[k * 3 + column]).sum();
        }
    }
    blobs::QTransform {
        m11: result[0],
        m12: result[1],
        m13: result[2],
        m21: result[3],
        m22: result[4],
        m23: result[5],
        m31: result[6],
        m32: result[7],
        m33: result[8],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn splice_reads_across_head_copy_without_allocating_database() {
        let mut source = Splice {
            source: Cursor::new(b"xxxxEFGHABCD"),
            db_len: 8,
            chunk: 4,
            position: 0,
        };
        let mut bytes = [0; 8];
        source.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ABCDEFGH");
        source.seek(SeekFrom::End(-2)).unwrap();
        let mut tail = [0; 2];
        source.read_exact(&mut tail).unwrap();
        assert_eq!(&tail, b"GH");
        assert!(source.seek(SeekFrom::Current(-9)).is_err());
    }
    #[test]
    fn composing_parent_rotation_preserves_child_translation() {
        let p = blobs::QTransform {
            m11: 0.,
            m12: 1.,
            m21: -1.,
            m22: 0.,
            m31: 10.,
            m32: 20.,
            ..blobs::QTransform::IDENTITY
        };
        let c = blobs::QTransform {
            m31: 3.,
            m32: 4.,
            ..blobs::QTransform::IDENTITY
        };
        let world = compose(p, c);
        assert_eq!((world.m31, world.m32), (6., 23.));
    }
    #[test]
    fn complete_legacy_files_cannot_exceed_text_depth_budget() {
        for depth in [31usize, 33] {
            let mut bytes = vec![0u8; 224 + depth * 158 + 12];
            bytes[..4].copy_from_slice(&8u32.to_be_bytes());
            for (i, c) in "1.10".encode_utf16().enumerate() {
                bytes[4 + i * 2..6 + i * 2].copy_from_slice(&c.to_be_bytes());
            }
            for i in 0..depth {
                let start = 224 + i * 158;
                bytes[start..start + 8].copy_from_slice(&((start + 158) as u64).to_be_bytes());
                bytes[start + 8..start + 12].copy_from_slice(&32u32.to_be_bytes());
                for (j, c) in "GraphicsTextItem".encode_utf16().enumerate() {
                    bytes[start + 12 + j * 2..start + 14 + j * 2].copy_from_slice(&c.to_be_bytes());
                }
                bytes[start + 154..start + 158]
                    .copy_from_slice(&u32::from(i + 1 < depth).to_be_bytes());
            }
            let digest = crate::checksum::md5_hex_from(&bytes, 108);
            for (i, c) in digest.bytes().enumerate() {
                bytes[45 + i * 2] = c;
            }
            let result = read(Cursor::new(bytes), &mut |_| Ok(()));
            assert_eq!(result.is_ok(), depth <= 32);
        }
    }
    #[test]
    fn four_gib_is_inclusive_and_larger_boards_are_rejected_before_reading() {
        struct Sized {
            bytes: Cursor<&'static [u8]>,
            length: u64,
        }
        impl Read for Sized {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                self.bytes.read(bytes)
            }
        }
        impl Seek for Sized {
            fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
                match position {
                    SeekFrom::End(0) => Ok(self.length),
                    position => self.bytes.seek(position),
                }
            }
        }
        for length in [MAX_FILE_BYTES, MAX_FILE_BYTES + 1] {
            let source = Sized {
                bytes: Cursor::new(include_bytes!("../../../corpus/pur/v1.10-redbar.pur")),
                length,
            };
            let result = read(source, &mut |_| {
                panic!("truncated virtual file must not emit")
            });
            if length == MAX_FILE_BYTES {
                assert!(matches!(result, Err(Error::Io(_))));
            } else {
                assert!(matches!(result, Err(Error::Budget(_))));
            }
        }
    }
    #[test]
    fn physical_column_order_is_read_from_schema() {
        let fixture = include_bytes!("../../../corpus/pur/v2.1-redbar.pur");
        let db_len = u64::from_be_bytes(fixture[14..22].try_into().unwrap());
        let chunk = 108 + u32::from_be_bytes(fixture[104..108].try_into().unwrap()) as u64;
        let mut splice = Splice {
            source: Cursor::new(fixture),
            db_len,
            chunk,
            position: 0,
        };
        let mut bytes = Vec::new();
        splice.read_to_end(&mut bytes).unwrap();
        let old=b"width INTEGER,checksum TEXT,format TEXT,source TEXT,height INTEGER,data BLOB,source_type INTEGER,origin TEXT,id INTEGER PRIMARY KEY";
        // Actual 2.1.2 layout found in the private FemaleFighterPosesPack board.
        let new=b"id INTEGER PRIMARY KEY,source_type INTEGER,origin TEXT,source TEXT,format TEXT,checksum TEXT,data BLOB,width INTEGER,height INTEGER";
        assert_eq!(old.len(), new.len());
        let offset = bytes.windows(old.len()).position(|w| w == old).unwrap();
        bytes[offset..offset + old.len()].copy_from_slice(new);
        let mut db = Database::open(Cursor::new(bytes)).unwrap();
        let (_, order) = table(
            &mut db,
            "images",
            &[
                "width",
                "checksum",
                "format",
                "source",
                "height",
                "data",
                "source_type",
                "origin",
                "id",
            ],
        )
        .unwrap();
        assert_eq!(order, [7, 5, 4, 3, 8, 6, 1, 2, 0]);
        let row = ordered(
            vec![
                Value::Null,
                Value::Integer(1),
                Value::Text("origin".into()),
                Value::Text("source".into()),
                Value::Text("JPG".into()),
                Value::Text("hash".into()),
                Value::Blob(vec![1, 2, 3]),
                Value::Integer(3840),
                Value::Integer(3585),
            ],
            &order,
        )
        .unwrap();
        assert_eq!(row[0].integer(), Some(3840));
        assert_eq!(row[4].integer(), Some(3585));
        assert_eq!(row[5], Value::Blob(vec![1, 2, 3]));
    }
}
