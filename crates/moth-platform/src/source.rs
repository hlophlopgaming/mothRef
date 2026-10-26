// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only file, ZIP-entry and journal-range sources share the import path.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::PathBuf,
};

#[derive(Clone)]
pub(crate) struct Source {
    pub path: PathBuf,
    pub range: Option<moth_format::board::Blob>,
    pub member: Option<String>,
}

impl Source {
    pub fn open(&self) -> io::Result<Box<dyn moth_format::ReadSeek>> {
        let mut file = File::open(&self.path)?;
        if let Some(member) = &self.member {
            if file.metadata()?.len() > moth_io::MAX_INPUT_BYTES * 16 {
                return Err(io::Error::other("ZIP archive exceeds 1 GiB"));
            }
            let mut archive = zip::ZipArchive::new(file).map_err(io::Error::other)?;
            let mut entry = archive.by_name(member).map_err(io::Error::other)?;
            if entry.is_dir() || entry.size() > moth_io::MAX_INPUT_BYTES {
                return Err(io::Error::other(
                    "ZIP image exceeds input limit or is a directory",
                ));
            }
            let mut bytes = Vec::new();
            (&mut entry)
                .take(moth_io::MAX_INPUT_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > moth_io::MAX_INPUT_BYTES {
                return Err(io::Error::other("ZIP image exceeds input limit"));
            }
            return Ok(Box::new(io::Cursor::new(bytes)));
        }
        let Some(range) = self.range else {
            return Ok(Box::new(file));
        };
        let end = range
            .offset
            .checked_add(range.length)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid original range"))?;
        if end > file.metadata()?.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated original range",
            ));
        }
        file.seek(SeekFrom::Start(range.offset))?;
        Ok(Box::new(Range {
            file,
            range,
            position: 0,
        }))
    }
}

struct Range {
    file: File,
    range: moth_format::board::Blob,
    position: u64,
}

impl Read for Range {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let length = (self.range.length - self.position).min(bytes.len() as u64) as usize;
        let count = self.file.read(&mut bytes[..length])?;
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for Range {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.range.length) + i128::from(offset),
        };
        if position < 0 || position > i128::from(self.range.length) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek outside original range",
            ));
        }
        self.file
            .seek(SeekFrom::Start(self.range.offset + position as u64))?;
        self.position = position as u64;
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_sources_read_only_the_selected_entry_and_reject_oversized_images() {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!("mothref-zip-{}.zip", uuid::Uuid::now_v7()));
        let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        archive.start_file("folder/кот#.png", options).unwrap();
        archive.write_all(b"original image bytes").unwrap();
        archive.start_file("large.png", options).unwrap();
        io::copy(
            &mut io::repeat(0).take(moth_io::MAX_INPUT_BYTES + 1),
            &mut archive,
        )
        .unwrap();
        archive.finish().unwrap();
        let store = crate::DesktopStorage::default();
        let handle = store
            .register_zip(path.clone(), "folder/кот#.png".into())
            .unwrap();
        use moth_format::Storage;
        let mut bytes = Vec::new();
        store.open(handle).unwrap().read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"original image bytes");
        assert_eq!(store.source_name(handle), "кот#.png");
        assert!(store.append(handle).is_err());
        assert!(store.sync(handle).is_err());
        let large = store
            .register_zip(path.clone(), "large.png".into())
            .unwrap();
        assert!(store.open(large).is_err());
        let missing = store
            .register_zip(path.clone(), "missing.png".into())
            .unwrap();
        assert!(store.open(missing).is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn ranges_isolate_reads_seeks_and_independent_cursors() {
        let path = std::env::temp_dir().join(format!("mothref-range-{}", uuid::Uuid::now_v7()));
        std::fs::write(&path, b"prefixIMAGEsuffix").unwrap();
        let source = Source {
            member: None,
            path: path.clone(),
            range: Some(moth_format::board::Blob {
                offset: 6,
                length: 5,
            }),
        };
        let mut first = source.open().unwrap();
        let mut second = source.open().unwrap();
        assert_eq!(first.seek(SeekFrom::End(0)).unwrap(), 5);
        assert_eq!(first.seek(SeekFrom::End(-2)).unwrap(), 3);
        let mut bytes = Vec::new();
        first.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"GE");
        assert_eq!(first.read(&mut [0; 10]).unwrap(), 0);
        assert!(first.seek(SeekFrom::Current(1)).is_err());
        assert!(first.seek(SeekFrom::Start(6)).is_err());
        assert!(first.seek(SeekFrom::End(-6)).is_err());
        assert_eq!(first.seek(SeekFrom::Current(-5)).unwrap(), 0);
        bytes.clear();
        second.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"IMAGE");
        drop(first);
        drop(second);
        std::fs::write(&path, b"short").unwrap();
        assert!(matches!(source.open(), Err(e) if e.kind() == io::ErrorKind::UnexpectedEof));
        let overflow = Source {
            member: None,
            path: path.clone(),
            range: Some(moth_format::board::Blob {
                offset: u64::MAX,
                length: 1,
            }),
        };
        assert!(matches!(overflow.open(), Err(e) if e.kind() == io::ErrorKind::InvalidInput));
        std::fs::remove_file(path).unwrap();
    }
}
