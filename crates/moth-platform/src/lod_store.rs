// SPDX-License-Identifier: GPL-3.0-or-later
//! Disk backend for the "Cache on disk" performance mode.
//!
//! Entries are disposable GPU-ready pyramids in the OS cache folder. They never hold
//! board data: deleting the folder only costs new decodes. The folder has a byte budget
//! and is pruned oldest-first by the decode workers, never by the event loop.
use moth_format::ReadSeek;
use std::{
    fs,
    io::{self, BufReader, Write},
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, SystemTime},
};

/// Default disk budget for cached pyramids.
pub const DISK_BUDGET: u64 = 2 * 1024 * 1024 * 1024;
/// Pruning scans the folder, so it runs on the first write and then periodically.
const PRUNE_EVERY: u32 = 32;
/// Temporary files older than this belong to an interrupted write.
const STALE_TEMPORARY: Duration = Duration::from_secs(60 * 60);

fn cache_directory() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base =
        std::env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("mothref/cache"));
    #[cfg(target_os = "macos")]
    let base =
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Caches/mothref"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|value| PathBuf::from(value).is_absolute())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .map(|path| path.join("mothref"));
    base.filter(|path| path.is_absolute())
        .map(|path| path.join("lod"))
}

/// LOD pyramids stored as one file per image and GPU format.
pub struct DiskStore {
    directory: Option<PathBuf>,
    budget: u64,
    writes: AtomicU32,
}
impl Default for DiskStore {
    fn default() -> Self {
        Self::new()
    }
}
impl DiskStore {
    /// Uses the OS cache folder; resolving it reads environment variables only.
    pub fn new() -> Self {
        Self {
            directory: cache_directory(),
            budget: DISK_BUDGET,
            writes: AtomicU32::new(0),
        }
    }
    /// Isolates tools and tests from the user's cache folder.
    pub fn in_directory(directory: PathBuf, budget: u64) -> Self {
        Self {
            directory: Some(directory),
            budget,
            writes: AtomicU32::new(0),
        }
    }
    fn path(&self, key: &str) -> io::Result<PathBuf> {
        if key.is_empty()
            || key.len() > 128
            || !key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid LOD cache key",
            ));
        }
        let directory = self.directory.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "cache directory unavailable")
        })?;
        Ok(directory.join(format!("{key}.lod")))
    }
    /// Removes interrupted writes, then the least recently used entries until the
    /// folder is back under 90% of its budget.
    fn prune(&self) -> io::Result<()> {
        let Some(directory) = &self.directory else {
            return Ok(());
        };
        let now = SystemTime::now();
        let mut entries = Vec::new();
        let mut total = 0u64;
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if !metadata.is_file() {
                continue;
            }
            let path = entry.path();
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            if extension == "lod" {
                total = total.saturating_add(metadata.len());
                entries.push((modified, metadata.len(), path));
            } else if extension == "tmp"
                && now
                    .duration_since(modified)
                    .is_ok_and(|age| age > STALE_TEMPORARY)
            {
                let _ = fs::remove_file(&path);
            }
        }
        entries.sort_by_key(|(modified, _, _)| *modified);
        let target = self.budget / 10 * 9;
        for (_, len, path) in entries {
            if total <= target {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(len);
            }
        }
        Ok(())
    }
}
impl moth_io::LodStore for DiskStore {
    fn read(&self, key: &str) -> io::Result<Option<Box<dyn ReadSeek>>> {
        let path = self.path(key)?;
        let file = match fs::File::options().read(true).write(true).open(&path) {
            Ok(file) => {
                // Reads refresh the age that pruning uses, making it least recently used.
                let _ = file.set_modified(SystemTime::now());
                file
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => match fs::File::open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error),
            },
        };
        let reader: Box<dyn ReadSeek> = Box::new(BufReader::new(file));
        Ok(Some(reader))
    }
    fn write(&self, key: &str, parts: &[&[u8]]) -> io::Result<()> {
        let path = self.path(key)?;
        let size = parts.iter().map(|p| p.len() as u64).sum::<u64>();
        if size > self.budget / 4 {
            return Ok(());
        }
        let directory = path
            .parent()
            .ok_or_else(|| io::Error::other("cache entry has no folder"))?;
        fs::create_dir_all(directory)?;
        if self
            .writes
            .fetch_add(1, Ordering::Relaxed)
            .is_multiple_of(PRUNE_EVERY)
            && let Err(error) = self.prune()
        {
            tracing::debug!(%error, "LOD cache pruning failed");
        }
        let temporary = directory.join(format!("{key}.{}.tmp", uuid::Uuid::now_v7()));
        let written = (|| {
            let mut file = fs::File::create_new(&temporary)?;
            for part in parts {
                file.write_all(part)?;
            }
            file.flush()?;
            drop(file);
            // Entries are rebuildable, so no fsync: a torn file fails its checksums.
            fs::rename(&temporary, &path)
        })();
        if written.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moth_io::LodStore;
    use std::io::Read;

    #[test]
    fn writes_replace_entries_and_reject_unsafe_keys() {
        let root = std::env::temp_dir().join(format!("mothref-lod-{}", uuid::Uuid::now_v7()));
        let store = DiskStore::in_directory(root.clone(), 1024 * 1024);
        assert!(store.read("abc-rgba8").unwrap().is_none());
        store
            .write("abc-rgba8", &[b"one".as_slice(), b"two".as_slice()])
            .unwrap();
        store.write("abc-rgba8", &[b"three".as_slice()]).unwrap();
        let mut text = String::new();
        store
            .read("abc-rgba8")
            .unwrap()
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "three");
        assert!(store.write("../escape", &[b"x".as_slice()]).is_err());
        assert!(store.read("A/B").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pruning_keeps_the_folder_under_budget() {
        let root = std::env::temp_dir().join(format!("mothref-lod-{}", uuid::Uuid::now_v7()));
        let store = DiskStore::in_directory(root.clone(), 4000);
        let payload = vec![7u8; 900];
        for i in 0..PRUNE_EVERY + 1 {
            store
                .write(&format!("entry-{i}"), &[payload.as_slice()])
                .unwrap();
        }
        let total: u64 = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().metadata().unwrap().len())
            .sum();
        // One prune ran after the budget filled; at most the entries written since remain.
        assert!(total <= 4000 + 900 * u64::from(PRUNE_EVERY));
        store.prune().unwrap();
        let total: u64 = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().metadata().unwrap().len())
            .sum();
        assert!(total <= 3600);
        fs::remove_dir_all(root).unwrap();
    }
}
