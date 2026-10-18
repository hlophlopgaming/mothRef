// SPDX-License-Identifier: GPL-3.0-or-later
//! Minimal read-only reader for the SQLite 3 file format.
//!
//! BeeRef stores `.bee` boards as a plain SQLite database. Pulling in a C
//! SQLite build would break the Android and WASM ports, so MothRef reads only
//! what an import needs: the header, table b-trees, the record format and
//! overflow chains. Every walk is bounds-checked and reports an error instead
//! of panicking on malformed input, because `.bee` files come from the user's
//! disk and may be corrupt.
//!
//! Reference: <https://www.sqlite.org/fileformat.html>
use std::io::{Read, Seek};

/// Portable byte sources need no native filesystem path.
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}
use std::io::SeekFrom;

const HEADER_LEN: usize = 100;
const MAGIC: &[u8; 16] = b"SQLite format 3\0";
const PAGE_LEAF_TABLE: u8 = 13;
const PAGE_INTERIOR_TABLE: u8 = 5;

/// Corrupt or unsupported input is reported, never treated as empty data.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Reading from the board source failed.
    #[error("sqlite I/O: {0}")]
    Io(#[from] std::io::Error),
    /// The file does not start with the SQLite magic string.
    #[error("not a SQLite 3 database")]
    NotSqlite,
    /// The header, a page or a record is inconsistent with the format.
    #[error("malformed SQLite database")]
    Malformed,
    /// A feature outside the read-only subset (for example UTF-16 text).
    #[error("unsupported SQLite feature: {0}")]
    Unsupported(&'static str),
}

/// A bounds-checked SQLite value as it appears in one record.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// SQL NULL, including the implicit value of an `INTEGER PRIMARY KEY`.
    Null,
    /// Any of the integer serial types.
    Integer(i64),
    /// IEEE-754 double.
    Real(f64),
    /// UTF-8 text.
    Text(String),
    /// Raw bytes, used for embedded images.
    Blob(Vec<u8>),
}

impl Value {
    /// Borrows text payloads without copying large JSON columns.
    pub fn text(&self) -> Option<&str> {
        if let Self::Text(value) = self {
            Some(value)
        } else {
            None
        }
    }
    /// Reads an integer column, ignoring any other type.
    pub fn integer(&self) -> Option<i64> {
        if let Self::Integer(value) = self {
            Some(*value)
        } else {
            None
        }
    }
    /// Reads a numeric column as `f64`; SQLite may store whole floats as ints.
    pub fn real(&self) -> Option<f64> {
        match self {
            Self::Real(value) => Some(*value),
            Self::Integer(value) => Some(*value as f64),
            _ => None,
        }
    }
}

/// Opens one database and walks its table b-trees.
pub struct Database<S> {
    source: S,
    page_size: u32,
    usable: usize,
    page_count: u32,
}

impl<S: ReadSeek> Database<S> {
    /// Validates the 100-byte header before any page is trusted.
    pub fn open(mut source: S) -> Result<Self, Error> {
        let mut header = [0u8; HEADER_LEN];
        source.seek(SeekFrom::Start(0))?;
        source.read_exact(&mut header)?;
        if &header[..16] != MAGIC {
            return Err(Error::NotSqlite);
        }
        let mut page_size = u16::from_be_bytes([header[16], header[17]]) as u32;
        if page_size == 1 {
            page_size = 65536;
        }
        if !(512..=65536).contains(&page_size) || !page_size.is_power_of_two() {
            return Err(Error::Malformed);
        }
        let reserved = header[20] as usize;
        if reserved >= page_size as usize {
            return Err(Error::Malformed);
        }
        let encoding = u32::from_be_bytes([header[56], header[57], header[58], header[59]]);
        if encoding > 1 {
            return Err(Error::Unsupported("UTF-16 text encoding"));
        }
        let file_len = source.seek(SeekFrom::End(0))?;
        let pages_in_file =
            u32::try_from(file_len / page_size as u64).map_err(|_| Error::Malformed)?;
        let stated = u32::from_be_bytes([header[28], header[29], header[30], header[31]]);
        let page_count = if stated == 0 {
            pages_in_file
        } else {
            stated.min(pages_in_file)
        };
        if page_count == 0 {
            return Err(Error::Malformed);
        }
        Ok(Self {
            source,
            page_size,
            usable: page_size as usize - reserved,
            page_count,
        })
    }

    /// Finds the root page of a table by scanning the schema b-tree on page 1.
    pub fn root_page(&mut self, table: &str) -> Result<Option<u32>, Error> {
        let mut found = None;
        self.walk_table(1, &mut |_rowid, values| -> Result<(), Error> {
            if values.get(1).and_then(Value::text) == Some(table)
                && let Some(Value::Integer(root)) = values.get(3)
            {
                found = Some(*root as u32);
            }
            Ok(())
        })?;
        Ok(found)
    }

    /// Qt versions create the same columns in different orders; import by name.
    pub fn columns(&mut self, table: &str) -> Result<Vec<String>, Error> {
        let mut columns = None;
        self.walk_table(1, &mut |_id, row| -> Result<(), Error> {
            if row.get(1).and_then(Value::text) == Some(table) {
                let sql = row.get(4).and_then(Value::text).ok_or(Error::Malformed)?;
                let (_, fields) = sql.split_once('(').ok_or(Error::Malformed)?;
                let (fields, _) = fields.rsplit_once(')').ok_or(Error::Malformed)?;
                let mut names = Vec::new();
                for field in fields.split(',') {
                    // shortcut: simple Qt column declarations; extend for a captured richer schema.
                    if field.contains(['(', ')']) {
                        return Err(Error::Unsupported("complex table declaration"));
                    }
                    let name = field.split_whitespace().next().ok_or(Error::Malformed)?;
                    let name = name.trim_matches(['"', '`', '[', ']']).to_ascii_lowercase();
                    if name.is_empty() || names.contains(&name) || names.len() >= 256 {
                        return Err(Error::Malformed);
                    }
                    names.push(name);
                }
                columns = Some(names);
            }
            Ok(())
        })?;
        columns.ok_or(Error::Malformed)
    }

    /// Walks a table b-tree, yielding `(rowid, columns)` for every leaf row.
    /// The callback's error type is threaded through, so callers keep their
    /// own error (the sink's I/O, for example) instead of wrapping it.
    pub fn walk_table<E: From<Error>>(
        &mut self,
        root: u32,
        rows: &mut dyn FnMut(i64, Vec<Value>) -> Result<(), E>,
    ) -> Result<(), E> {
        let page = self.read_page(root)?;
        let body = if root == 1 { HEADER_LEN } else { 0 };
        let mut remaining = self.page_count;
        self.walk_page(&page, body, 0, &mut remaining, rows)
    }

    fn walk_page<E: From<Error>>(
        &mut self,
        page: &[u8],
        body: usize,
        depth: usize,
        remaining: &mut u32,
        rows: &mut dyn FnMut(i64, Vec<Value>) -> Result<(), E>,
    ) -> Result<(), E> {
        // Bound both stack use and repeated visits through corrupt child pointers.
        if depth >= 64 || *remaining == 0 {
            return Err(Error::Malformed.into());
        }
        *remaining -= 1;
        let kind = *page.get(body).ok_or(Error::Malformed)?;
        let header_size = match kind {
            PAGE_LEAF_TABLE => 8,
            PAGE_INTERIOR_TABLE => 12,
            _ => return Err(Error::Malformed.into()),
        };
        let count = u16_at(page, body + 3)? as usize;
        let cell_array = body + header_size;
        match kind {
            PAGE_LEAF_TABLE => {
                for i in 0..count {
                    let offset = u16_at(page, cell_array + i * 2)? as usize;
                    let (payload, rowid) = self.leaf_cell(page, offset)?;
                    rows(rowid, decode_record(&payload)?)?;
                }
            }
            PAGE_INTERIOR_TABLE => {
                let mut children = Vec::with_capacity(count + 1);
                for i in 0..count {
                    let offset = u16_at(page, cell_array + i * 2)? as usize;
                    children.push(u32_at(page, offset)?);
                }
                children.push(u32_at(page, body + 8)?);
                for child in children {
                    let child_page = self.read_page(child)?;
                    let child_body = if child == 1 { HEADER_LEN } else { 0 };
                    self.walk_page(&child_page, child_body, depth + 1, remaining, rows)?;
                }
            }
            _ => return Err(Error::Malformed.into()),
        }
        Ok(())
    }

    fn read_page(&mut self, number: u32) -> Result<Vec<u8>, Error> {
        if number == 0 || number > self.page_count {
            return Err(Error::Malformed);
        }
        let mut page = vec![0u8; self.page_size as usize];
        self.source
            .seek(SeekFrom::Start((number as u64 - 1) * self.page_size as u64))?;
        self.source.read_exact(&mut page)?;
        Ok(page)
    }

    /// Parses one table-leaf cell, stitching the payload across overflow pages.
    fn leaf_cell(&mut self, page: &[u8], offset: usize) -> Result<(Vec<u8>, i64), Error> {
        let mut position = offset;
        let payload_len =
            usize::try_from(varint(page, &mut position)?).map_err(|_| Error::Malformed)?;
        if payload_len > 32 * 1024 * 1024 {
            return Err(Error::Unsupported("record exceeds 32 MiB import budget"));
        }
        let rowid = varint(page, &mut position)? as i64;
        let usable = self.usable;
        let local_limit = usable - 35;
        let local = if payload_len <= local_limit {
            payload_len
        } else {
            let minimum = ((usable - 12) * 32 / 255) - 23;
            let candidate = minimum + (payload_len - minimum) % (usable - 4);
            if candidate <= local_limit {
                candidate
            } else {
                minimum
            }
        };
        let end = position.checked_add(local).ok_or(Error::Malformed)?;
        let mut payload = page.get(position..end).ok_or(Error::Malformed)?.to_vec();
        if payload.len() < payload_len {
            let mut next = u32_at(page, end)?;
            while payload.len() < payload_len {
                if next == 0 {
                    return Err(Error::Malformed);
                }
                let overflow = self.read_page(next)?;
                let take = (payload_len - payload.len()).min(usable - 4);
                payload.extend_from_slice(overflow.get(4..4 + take).ok_or(Error::Malformed)?);
                next = u32_at(&overflow, 0)?;
            }
        }
        Ok((payload, rowid))
    }
}

/// Decodes a record body into its column values, in declared order.
fn decode_record(payload: &[u8]) -> Result<Vec<Value>, Error> {
    let mut position = 0usize;
    let header_len = varint(payload, &mut position)? as usize;
    if header_len < position || header_len > payload.len() {
        return Err(Error::Malformed);
    }
    let mut serial_types = Vec::new();
    while position < header_len {
        if serial_types.len() >= 256 {
            return Err(Error::Malformed);
        }
        serial_types.push(varint(payload, &mut position)?);
    }
    let mut body = header_len;
    let mut values = Vec::with_capacity(serial_types.len());
    for serial in serial_types {
        let (value, consumed) = decode_value(serial, body, payload)?;
        body += consumed;
        values.push(value);
    }
    Ok(values)
}

/// Maps one SQLite serial type and body slice to a value and its byte length.
fn decode_value(serial: u64, body: usize, payload: &[u8]) -> Result<(Value, usize), Error> {
    let slice = |len: usize| -> Result<&[u8], Error> {
        let end = body.checked_add(len).ok_or(Error::Malformed)?;
        payload.get(body..end).ok_or(Error::Malformed)
    };
    let integer = |len: usize| -> Result<i64, Error> {
        let bytes = slice(len)?;
        if len == 0 {
            return Ok(0);
        }
        let sign = if bytes[0] & 0x80 != 0 { -1i64 } else { 0 };
        let mut value = sign;
        for &byte in bytes {
            value = (value << 8) | i64::from(byte);
        }
        Ok(value)
    };
    Ok(match serial {
        0 => (Value::Null, 0),
        1 => (Value::Integer(integer(1)?), 1),
        2 => (Value::Integer(integer(2)?), 2),
        3 => (Value::Integer(integer(3)?), 3),
        4 => (Value::Integer(integer(4)?), 4),
        5 => (Value::Integer(integer(6)?), 6),
        6 => (
            Value::Integer(i64::from_be_bytes(
                slice(8)?.try_into().map_err(|_| Error::Malformed)?,
            )),
            8,
        ),
        7 => (
            Value::Real(f64::from_be_bytes(
                slice(8)?.try_into().map_err(|_| Error::Malformed)?,
            )),
            8,
        ),
        8 => (Value::Integer(0), 0),
        9 => (Value::Integer(1), 0),
        10 | 11 => return Err(Error::Malformed),
        n if n % 2 == 0 => {
            let len = ((n - 12) / 2) as usize;
            (Value::Blob(slice(len)?.to_vec()), len)
        }
        n => {
            let len = ((n - 13) / 2) as usize;
            (
                Value::Text(String::from_utf8_lossy(slice(len)?).into_owned()),
                len,
            )
        }
    })
}

/// SQLite varints are big-endian base-128; the ninth byte uses all eight bits.
fn varint(bytes: &[u8], position: &mut usize) -> Result<u64, Error> {
    let mut value = 0u64;
    for index in 0..9 {
        let byte = *bytes.get(*position).ok_or(Error::Malformed)?;
        *position += 1;
        if index == 8 {
            return Ok((value << 8) | u64::from(byte));
        }
        value = (value << 7) | u64::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(Error::Malformed)
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    let end = offset.checked_add(2).ok_or(Error::Malformed)?;
    Ok(u16::from_be_bytes(
        bytes
            .get(offset..end)
            .ok_or(Error::Malformed)?
            .try_into()
            .map_err(|_| Error::Malformed)?,
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    let end = offset.checked_add(4).ok_or(Error::Malformed)?;
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..end)
            .ok_or(Error::Malformed)?
            .try_into()
            .map_err(|_| Error::Malformed)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn table_depth_is_bounded() {
        for page_count in [64u32, 65] {
            let mut bytes = vec![0u8; page_count as usize * 512];
            for number in 1..=page_count {
                let body = (number as usize - 1) * 512 + if number == 1 { HEADER_LEN } else { 0 };
                if number == page_count {
                    bytes[body] = PAGE_LEAF_TABLE;
                } else {
                    bytes[body] = PAGE_INTERIOR_TABLE;
                    bytes[body + 8..body + 12].copy_from_slice(&(number + 1).to_be_bytes());
                }
            }
            let mut database = Database {
                source: Cursor::new(bytes),
                page_size: 512,
                usable: 512,
                page_count,
            };
            let result = database.walk_table(1, &mut |_, _| -> Result<(), Error> { Ok(()) });
            assert_eq!(result.is_ok(), page_count == 64);
        }
    }

    #[test]
    fn arbitrary_bytes_never_panic() {
        for len in [0usize, 1, 16, 100, 512, 4096] {
            let bytes = vec![0x5a; len];
            let _ = Database::open(Cursor::new(bytes));
        }
    }

    #[test]
    fn wrong_magic_is_rejected() {
        let mut header = vec![0u8; 512];
        header[..16].copy_from_slice(b"not a database!\0");
        assert!(matches!(
            Database::open(Cursor::new(header)),
            Err(Error::NotSqlite)
        ));
    }
}
