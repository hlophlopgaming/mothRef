// SPDX-License-Identifier: GPL-3.0-or-later
//! Tile packs: every detail tile of one image in one rebuildable cache file.
//!
//! A background bake writes the pack once; zoom then reads GPU-ready tiles from it
//! instead of decoding the original. One file per image keeps the cache folder small
//! enough for its pruning scan. Like every LOD cache entry the file is untrusted on
//! read: sizes are checked before allocation and each tile carries a blake3 digest.
//!
//! Layout: tile payloads (fixed size per GPU format) in bake order, then an index of
//! entries sorted by tile, then a fixed trailer.
use crate::{
    Decoded, ImageFormat, Mip,
    cache::{format_code, mip_bytes},
    tiles::{SIDE, Tile},
};
use moth_format::ReadSeek;
use std::io::{self, Read, Seek, SeekFrom, Write};

const MAGIC: &[u8; 8] = b"MOTHTIL1";
/// Level, x, y, payload offset, payload digest.
const ENTRY: u64 = 4 + 4 + 4 + 8 + 32;
/// Magic, format, original width and height, tile count, index offset, content hash.
const TRAILER: u64 = 8 + 4 + 4 + 4 + 4 + 8 + 32;

/// Cache key of an image's pack; distinct from its whole-image pyramid key.
pub(crate) fn key(hash: blake3::Hash, format: ImageFormat) -> String {
    let suffix = match format {
        ImageFormat::Rgba8 => "rgba8",
        ImageFormat::Bc7 => "bc7",
    };
    format!("{}-tiles-{suffix}", hash.to_hex())
}
/// Bytes of one stored tile.
pub(crate) fn payload(format: ImageFormat) -> u64 {
    mip_bytes(format, SIDE, SIDE)
}
/// Exact file size of a pack holding `tiles` tiles.
pub(crate) fn size(format: ImageFormat, tiles: u64) -> u64 {
    tiles * (payload(format) + ENTRY) + TRAILER
}
fn invalid(reason: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason.to_owned())
}

/// Appends tiles as they are baked and writes the index last.
pub(crate) struct PackWriter<W: Write> {
    out: W,
    format: ImageFormat,
    offset: u64,
    entries: Vec<(Tile, u64, [u8; 32])>,
}
impl<W: Write> PackWriter<W> {
    pub(crate) fn new(out: W, format: ImageFormat) -> Self {
        Self {
            out,
            format,
            offset: 0,
            entries: Vec::new(),
        }
    }
    /// Writes one finished tile in the pack's GPU format.
    pub(crate) fn push(&mut self, image: &Decoded) -> io::Result<()> {
        let tile = image
            .tile
            .ok_or_else(|| invalid("pack entry is not a tile"))?;
        let [mip] = image.mips.as_slice() else {
            return Err(invalid("pack tile must have exactly one level"));
        };
        if image.format != self.format
            || mip.width != SIDE
            || mip.height != SIDE
            || mip.bytes.len() as u64 != payload(self.format)
        {
            return Err(invalid("pack tile has the wrong format or size"));
        }
        if self.entries.len() as u64 >= crate::tiles::MAX_BAKED_TILES {
            return Err(invalid("pack has too many tiles"));
        }
        self.out.write_all(&mip.bytes)?;
        self.entries
            .push((tile, self.offset, *blake3::hash(&mip.bytes).as_bytes()));
        self.offset += mip.bytes.len() as u64;
        Ok(())
    }
    /// Writes the sorted index and trailer and returns the output for committing.
    pub(crate) fn finish(mut self, hash: blake3::Hash, original: [u32; 2]) -> io::Result<W> {
        self.entries.sort_by_key(|entry| entry.0);
        if self.entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(invalid("pack has a duplicate tile"));
        }
        let mut index = Vec::with_capacity(self.entries.len() * ENTRY as usize);
        for (tile, offset, digest) in &self.entries {
            index.extend_from_slice(&tile.level.to_le_bytes());
            index.extend_from_slice(&tile.x.to_le_bytes());
            index.extend_from_slice(&tile.y.to_le_bytes());
            index.extend_from_slice(&offset.to_le_bytes());
            index.extend_from_slice(digest);
        }
        self.out.write_all(&index)?;
        let mut trailer = Vec::with_capacity(TRAILER as usize);
        trailer.extend_from_slice(MAGIC);
        trailer.extend_from_slice(&u32::from(format_code(self.format)).to_le_bytes());
        trailer.extend_from_slice(&original[0].to_le_bytes());
        trailer.extend_from_slice(&original[1].to_le_bytes());
        trailer.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        trailer.extend_from_slice(&self.offset.to_le_bytes());
        trailer.extend_from_slice(hash.as_bytes());
        self.out.write_all(&trailer)?;
        self.out.flush()?;
        Ok(self.out)
    }
}

struct Trailer {
    original: [u32; 2],
    count: u64,
    index: u64,
}
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}
fn long(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"))
}
fn trailer(
    reader: &mut dyn ReadSeek,
    hash: blake3::Hash,
    format: ImageFormat,
) -> io::Result<Trailer> {
    let len = reader.seek(SeekFrom::End(0))?;
    if len < TRAILER {
        return Err(invalid("pack is truncated"));
    }
    reader.seek(SeekFrom::Start(len - TRAILER))?;
    let mut bytes = [0u8; TRAILER as usize];
    reader.read_exact(&mut bytes)?;
    let original = [word(&bytes, 12), word(&bytes, 16)];
    let count = u64::from(word(&bytes, 20));
    let index = long(&bytes, 24);
    if &bytes[..8] != MAGIC
        || word(&bytes, 8) != u32::from(format_code(format))
        || bytes[32..64] != hash.as_bytes()[..]
    {
        return Err(invalid("pack identity mismatch"));
    }
    if original.iter().any(|v| *v == 0 || *v > 32768)
        || count > crate::tiles::MAX_BAKED_TILES
        || index != count * payload(format)
        || index + count * ENTRY + TRAILER != len
    {
        return Err(invalid("pack layout is inconsistent"));
    }
    Ok(Trailer {
        original,
        count,
        index,
    })
}
/// Whether a pack is complete and belongs to this image and format.
pub(crate) fn check(
    mut reader: Box<dyn ReadSeek>,
    hash: blake3::Hash,
    format: ImageFormat,
) -> io::Result<()> {
    trailer(reader.as_mut(), hash, format).map(|_| ())
}
/// Reads one tile with a binary search over the index; `None` when the pack lacks it.
pub(crate) fn read_tile(
    mut reader: Box<dyn ReadSeek>,
    hash: blake3::Hash,
    format: ImageFormat,
    tile: Tile,
) -> io::Result<Option<Decoded>> {
    let info = trailer(reader.as_mut(), hash, format)?;
    let (mut lo, mut hi) = (0, info.count);
    let mut entry = [0u8; ENTRY as usize];
    let found = loop {
        if lo >= hi {
            break None;
        }
        let mid = lo + (hi - lo) / 2;
        reader.seek(SeekFrom::Start(info.index + mid * ENTRY))?;
        reader.read_exact(&mut entry)?;
        let here = Tile {
            level: word(&entry, 0),
            x: word(&entry, 4),
            y: word(&entry, 8),
        };
        match here.cmp(&tile) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => break Some(long(&entry, 12)),
        }
    };
    let Some(offset) = found else {
        return Ok(None);
    };
    let len = payload(format);
    if !offset.is_multiple_of(len) || offset + len > info.index {
        return Err(invalid("pack entry points outside its payloads"));
    }
    reader.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0u8; len as usize];
    reader.read_exact(&mut bytes)?;
    if blake3::hash(&bytes).as_bytes()[..] != entry[20..52] {
        return Err(invalid("pack tile checksum mismatch"));
    }
    Ok(Some(Decoded {
        tile: Some(tile),
        hash,
        original: info.original,
        mips: vec![Mip {
            width: SIDE,
            height: SIDE,
            bytes,
        }],
        format,
    }))
}

/// Reads every tile of a pack in file order and hands each to `emit`.
///
/// The index is read first (bounded by the tile limit), then payloads are read
/// sequentially, each checked against its digest before it is emitted.
pub(crate) fn read_all(
    mut reader: Box<dyn ReadSeek>,
    hash: blake3::Hash,
    format: ImageFormat,
    emit: &mut dyn FnMut(Decoded) -> io::Result<()>,
) -> io::Result<()> {
    let info = trailer(reader.as_mut(), hash, format)?;
    reader.seek(SeekFrom::Start(info.index))?;
    let mut index = vec![0u8; (info.count * ENTRY) as usize];
    reader.read_exact(&mut index)?;
    let len = payload(format);
    let mut entries = Vec::with_capacity(info.count as usize);
    for entry in index.as_chunks::<{ ENTRY as usize }>().0 {
        let offset = long(entry, 12);
        if !offset.is_multiple_of(len) || offset + len > info.index {
            return Err(invalid("pack entry points outside its payloads"));
        }
        let tile = Tile {
            level: word(entry, 0),
            x: word(entry, 4),
            y: word(entry, 8),
        };
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&entry[20..52]);
        entries.push((offset, tile, digest));
    }
    drop(index);
    entries.sort_by_key(|entry| entry.0);
    reader.seek(SeekFrom::Start(0))?;
    let mut position = 0;
    for (offset, tile, digest) in entries {
        if offset != position {
            reader.seek(SeekFrom::Start(offset))?;
        }
        let mut bytes = vec![0u8; len as usize];
        reader.read_exact(&mut bytes)?;
        position = offset + len;
        if *blake3::hash(&bytes).as_bytes() != digest {
            return Err(invalid("pack tile checksum mismatch"));
        }
        emit(Decoded {
            tile: Some(tile),
            hash,
            original: info.original,
            mips: vec![Mip {
                width: SIDE,
                height: SIDE,
                bytes,
            }],
            format,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn tile(level: u32, x: u32, y: u32, fill: u8) -> Decoded {
        Decoded {
            tile: Some(Tile { level, x, y }),
            hash: blake3::hash(b"pack"),
            original: [3000, 1000],
            mips: vec![Mip {
                width: SIDE,
                height: SIDE,
                bytes: vec![fill; payload(ImageFormat::Rgba8) as usize],
            }],
            format: ImageFormat::Rgba8,
        }
    }

    #[test]
    fn packs_round_trip_every_tile_and_reject_damage() {
        let hash = blake3::hash(b"pack");
        let tiles = [(1, 2, 0), (0, 0, 0), (0, 11, 3), (2, 0, 0), (0, 1, 0)];
        let mut writer = PackWriter::new(Vec::new(), ImageFormat::Rgba8);
        for (fill, &(level, x, y)) in tiles.iter().enumerate() {
            writer.push(&tile(level, x, y, fill as u8)).unwrap();
        }
        let bytes = writer.finish(hash, [3000, 1000]).unwrap();
        assert_eq!(
            bytes.len() as u64,
            size(ImageFormat::Rgba8, tiles.len() as u64)
        );
        let open = |bytes: &Vec<u8>| Box::new(Cursor::new(bytes.clone())) as Box<dyn ReadSeek>;
        check(open(&bytes), hash, ImageFormat::Rgba8).unwrap();
        for (fill, &(level, x, y)) in tiles.iter().enumerate() {
            let wanted = Tile { level, x, y };
            let image = read_tile(open(&bytes), hash, ImageFormat::Rgba8, wanted)
                .unwrap()
                .unwrap();
            assert_eq!(image.tile, Some(wanted));
            assert_eq!(image.original, [3000, 1000]);
            assert!(image.mips[0].bytes.iter().all(|v| *v == fill as u8));
        }
        let missing = Tile {
            level: 0,
            x: 5,
            y: 5,
        };
        assert!(
            read_tile(open(&bytes), hash, ImageFormat::Rgba8, missing)
                .unwrap()
                .is_none()
        );
        assert!(check(open(&bytes), blake3::hash(b"other"), ImageFormat::Rgba8).is_err());
        assert!(check(open(&bytes), hash, ImageFormat::Bc7).is_err());
        let mut damaged = bytes.clone();
        damaged[7] ^= 1;
        let first = Tile {
            level: 1,
            x: 2,
            y: 0,
        };
        assert!(read_tile(open(&damaged), hash, ImageFormat::Rgba8, first).is_err());
        let mut seen = Vec::new();
        read_all(open(&bytes), hash, ImageFormat::Rgba8, &mut |image| {
            seen.push((image.tile.unwrap(), image.mips[0].bytes[0]));
            Ok(())
        })
        .unwrap();
        // File order is bake order.
        let expected: Vec<_> = tiles
            .iter()
            .enumerate()
            .map(|(fill, &(level, x, y))| (Tile { level, x, y }, fill as u8))
            .collect();
        assert_eq!(seen, expected);
        assert!(read_all(open(&damaged), hash, ImageFormat::Rgba8, &mut |_| Ok(())).is_err());
        let truncated = bytes[..bytes.len() - 1].to_vec();
        assert!(check(open(&truncated), hash, ImageFormat::Rgba8).is_err());
    }

    #[test]
    fn writers_refuse_duplicates_and_foreign_formats() {
        let mut writer = PackWriter::new(Vec::new(), ImageFormat::Rgba8);
        writer.push(&tile(0, 0, 0, 1)).unwrap();
        writer.push(&tile(0, 0, 0, 2)).unwrap();
        assert!(writer.finish(blake3::hash(b"pack"), [3000, 1000]).is_err());
        let mut writer = PackWriter::new(Vec::new(), ImageFormat::Bc7);
        assert!(writer.push(&tile(0, 0, 0, 1)).is_err());
    }
}
