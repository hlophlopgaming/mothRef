// SPDX-License-Identifier: GPL-3.0-or-later
//! Disk backend for the "Cache on disk" performance mode.
//!
//! Entries are disposable GPU-ready pyramids in the OS cache folder. They never hold
//! board data: deleting the folder only costs new decodes. The folder has a byte budget
//! and is pruned oldest-first by the decode workers, never by the event loop.
use moth_format::ReadSeek;
use std::{
    fs,
    io::{self, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, SystemTime},
};

/// Default disk budget for cached pyramids and tile packs. Packs hold every detail
/// tile of a large image (about a third of its pixels in BC7), so the folder is larger
/// than the RAM cache it backs; pruning keeps it bounded.
pub const DISK_BUDGET: u64 = 8 * 1024 * 1024 * 1024;
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
    /// Samples rebuildable entries on a worker, never on the event loop.
    pub fn usage(&self) -> io::Result<u64> {
        self.entries(false)
    }
    /// Removes completed cache entries; concurrent decodes may populate new entries.
    pub fn clear(&self) -> io::Result<u64> {
        self.entries(true)
    }
    fn entries(&self, clear: bool) -> io::Result<u64> {
        let Some(directory) = &self.directory else {
            return Ok(0);
        };
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(error),
        };
        let mut bytes = 0u64;
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_file() || entry.path().extension().is_none_or(|e| e != "lod")
            {
                continue;
            }
            if clear {
                match fs::remove_file(entry.path()) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            } else {
                match entry.metadata() {
                    Ok(metadata) => bytes = bytes.saturating_add(metadata.len()),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(bytes)
    }
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

/// Bounded cache maintenance keeps directory scans and deletion off the UI thread.
pub struct Maintenance {
    requests: crossbeam_channel::Sender<bool>,
    results: crossbeam_channel::Receiver<Result<u64, String>>,
    last: Option<std::time::Instant>,
    /// True until the previous scan or clear completes.
    pub busy: bool,
    /// Last sampled size or filesystem error, for the settings panel.
    pub usage: Option<Result<u64, String>>,
}
impl Maintenance {
    /// Shares the exact disk backend used by the decode pools.
    pub fn new(
        store: std::sync::Arc<DiskStore>,
        wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        let (requests, rx) = crossbeam_channel::bounded(1);
        let (tx, results) = crossbeam_channel::bounded(1);
        std::thread::Builder::new()
            .name("lod-maintenance".into())
            .spawn(move || {
                while let Ok(clear) = rx.recv() {
                    let result = if clear { store.clear() } else { store.usage() }
                        .map_err(|e| e.to_string());
                    if tx.send(result).is_err() {
                        break;
                    }
                    wake();
                }
            })?;
        Ok(Self {
            requests,
            results,
            last: None,
            busy: false,
            usage: None,
        })
    }
    /// Samples only while settings are open; a clear is queued once explicitly requested.
    pub fn update(&mut self, visible: bool, clear: bool) -> bool {
        while let Ok(result) = self.results.try_recv() {
            self.usage = Some(result);
            self.busy = false;
        }
        if !self.busy
            && (clear
                || (visible
                    && self
                        .last
                        .is_none_or(|t| t.elapsed() >= Duration::from_secs(2))))
            && self.requests.try_send(clear).is_ok()
        {
            self.busy = true;
            self.last = Some(std::time::Instant::now());
            return clear;
        }
        false
    }
}
impl DiskStore {
    /// Creates the folder, prunes periodically and names a unique temporary file.
    fn prepare(&self, key: &str, path: &Path) -> io::Result<PathBuf> {
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
        Ok(directory.join(format!("{key}.{}.tmp", uuid::Uuid::now_v7())))
    }
}

/// A streamed entry written to a temporary file and renamed into place on commit.
struct DiskWriter {
    file: Option<BufWriter<fs::File>>,
    temporary: PathBuf,
    path: PathBuf,
}
impl Write for DiskWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("cache entry already committed"))?
            .write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        match self.file.as_mut() {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}
impl moth_io::LodWriter for DiskWriter {
    fn commit(mut self: Box<Self>) -> io::Result<()> {
        let file = self
            .file
            .take()
            .ok_or_else(|| io::Error::other("cache entry already committed"))?;
        let mut file = file.into_inner().map_err(io::IntoInnerError::into_error)?;
        file.flush()?;
        drop(file);
        // Entries are rebuildable, so no fsync: a torn file fails its checksums.
        fs::rename(&self.temporary, &self.path)
    }
}
impl Drop for DiskWriter {
    fn drop(&mut self) {
        // After a commit the temporary is already renamed and this removal is a no-op.
        drop(self.file.take());
        let _ = fs::remove_file(&self.temporary);
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
        let temporary = self.prepare(key, &path)?;
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
    fn writer(&self, key: &str, size: u64) -> io::Result<Option<Box<dyn moth_io::LodWriter>>> {
        let path = self.path(key)?;
        if size > self.budget / 4 {
            return Ok(None);
        }
        let temporary = self.prepare(key, &path)?;
        let file = fs::File::create_new(&temporary)?;
        Ok(Some(Box::new(DiskWriter {
            file: Some(BufWriter::with_capacity(256 * 1024, file)),
            temporary,
            path,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moth_io::LodStore;
    use std::io::Read;

    #[test]
    fn reopening_preloads_from_disk_without_originals_or_duplicate_entries() {
        use moth_format::{Handle, Storage};
        use moth_io::{CacheMode, DecodePool, ImageFormat, LodCache};
        use std::sync::Arc;

        struct Source(Option<Vec<u8>>);
        impl Storage for Source {
            fn open(&self, _: Handle) -> io::Result<Box<dyn ReadSeek>> {
                self.0
                    .clone()
                    .map(|bytes| Box::new(io::Cursor::new(bytes)) as Box<dyn ReadSeek>)
                    .ok_or_else(|| io::Error::other("reopening must not read the original"))
            }
            fn append(&self, _: Handle) -> io::Result<Box<dyn Write + Send>> {
                Err(io::ErrorKind::Unsupported.into())
            }
            fn sync(&self, _: Handle) -> io::Result<()> {
                Ok(())
            }
        }

        let root = std::env::temp_dir().join(format!("mothref-lod-{}", uuid::Uuid::now_v7()));
        let bytes = include_bytes!("../../../corpus/images/budget.webp");
        let hash = blake3::hash(bytes);
        let original = moth_io::decode(bytes, 256).unwrap().original;
        let count = 1 + moth_io::tiles::tile_count(
            original,
            moth_io::tiles::tiled_levels(original, CacheMode::Full.tile_threshold()),
        ) as usize;
        assert!(count > 1, "fixture must exercise tile packs too");
        let snapshot = || {
            let mut entries: Vec<_> = fs::read_dir(&root)
                .unwrap()
                .map(|entry| {
                    let path = entry.unwrap().path();
                    (
                        path.file_name().unwrap().to_owned(),
                        fs::read(path).unwrap(),
                    )
                })
                .collect();
            entries.sort();
            entries
        };
        let mut first = None;
        let mut files = None;
        for source in [Some(bytes.to_vec()), None] {
            let cache = Arc::new(LodCache::new());
            cache.set_mode(CacheMode::Full);
            cache.set_store(Arc::new(DiskStore::in_directory(root.clone(), DISK_BUDGET)));
            let pool =
                DecodePool::with_cache(Arc::new(Source(source)), Arc::new(|| {}), cache).unwrap();
            pool.preload(ImageFormat::Rgba8, [(Handle(1), hash)])
                .unwrap();
            let mut images = Vec::new();
            for _ in 0..count {
                let image = pool
                    .preloaded()
                    .recv_timeout(Duration::from_secs(30))
                    .unwrap();
                images.push((image.tile, image.original, image.mips[0].bytes.clone()));
            }
            images.sort_by_key(|image| image.0);
            if let Some(first) = &first {
                assert_eq!(&images, first);
                assert_eq!(snapshot(), files.unwrap());
                break;
            }
            first = Some(images);
            files = Some(snapshot());
            assert_eq!(files.as_ref().unwrap().len(), 2);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn usage_and_clear_only_touch_completed_cache_entries() {
        let root = std::env::temp_dir().join(format!("mothref-lod-{}", uuid::Uuid::now_v7()));
        let store = DiskStore::in_directory(root.clone(), 1024 * 1024);
        assert_eq!(store.usage().unwrap(), 0);
        assert_eq!(store.clear().unwrap(), 0);
        store.write("abc-rgba8", &[b"pixels"]).unwrap();
        fs::write(root.join("active.tmp"), b"in progress").unwrap();
        fs::write(root.join("keep.moth"), b"board").unwrap();
        fs::create_dir(root.join("nested.lod")).unwrap();
        assert_eq!(store.usage().unwrap(), 6);
        assert_eq!(store.clear().unwrap(), 0);
        assert_eq!(store.usage().unwrap(), 0);
        assert!(root.join("active.tmp").exists());
        assert!(root.join("keep.moth").exists());
        assert!(root.join("nested.lod").is_dir());
        fs::remove_dir_all(root).unwrap();
    }

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
    fn streamed_entries_appear_only_when_committed() {
        let root = std::env::temp_dir().join(format!("mothref-lod-{}", uuid::Uuid::now_v7()));
        let store = DiskStore::in_directory(root.clone(), 1024 * 1024);
        let mut writer = store.writer("abc-tiles-rgba8", 6).unwrap().unwrap();
        writer.write_all(b"tiles!").unwrap();
        assert!(store.read("abc-tiles-rgba8").unwrap().is_none());
        writer.commit().unwrap();
        let mut text = String::new();
        store
            .read("abc-tiles-rgba8")
            .unwrap()
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "tiles!");
        let mut abandoned = store.writer("def-tiles-rgba8", 3).unwrap().unwrap();
        abandoned.write_all(b"cut").unwrap();
        drop(abandoned);
        assert!(store.read("def-tiles-rgba8").unwrap().is_none());
        assert!(
            fs::read_dir(&root).unwrap().all(|e| e
                .unwrap()
                .path()
                .extension()
                .is_some_and(|e| e == "lod"))
        );
        assert!(
            store
                .writer("big-tiles-rgba8", 1024 * 1024)
                .unwrap()
                .is_none()
        );
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
