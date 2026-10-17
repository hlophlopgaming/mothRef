// SPDX-License-Identifier: GPL-3.0-or-later
//! Serializes durable mutations on a bounded storage queue.
use super::DesktopStorage;
use moth_format::{Handle, Storage, board};
use moth_scene::{BlobId, Command, Scene};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

/// Persistence intent; native paths never cross into the core crates.
pub enum Request {
    /// Original bytes are copied before the insert is acknowledged.
    Apply(Command, Option<Handle>),
    /// Journal the inverse before acknowledging it.
    Undo,
    /// Journal the previously undone command.
    Redo,
    /// Append an atomic checkpoint, keeping the old checkpoint intact.
    Save(PathBuf),
    /// Load into an empty board and establish a new recovery journal.
    Open(PathBuf),
}
/// Worker completions are delivered only after durable writes.
pub enum Completion {
    /// Offers existing journals without silently replacing the current board.
    Recovery(Vec<PathBuf>),
    /// Read-only recent-file suggestions, bounded to twenty entries.
    Recent(Vec<PathBuf>),
    /// Applies an already-journaled mutation on the UI thread.
    Applied(Command, Option<(BlobId, Handle)>),
    /// Replaces an empty scene with recovered metadata and staged source identities.
    Opened(Scene, HashMap<BlobId, Handle>, bool),
    /// Confirms an explicit save has reached stable storage.
    Saved,
    /// Leaves the UI state unchanged on failure.
    Failed(String),
}
/// Storage work is bounded in both directions and never blocks a frame.
pub struct Worker {
    /// Backpressure lets the UI report a busy storage worker.
    pub requests: crossbeam_channel::Sender<Request>,
    /// Poll after a wake event, never wait during rendering.
    pub results: crossbeam_channel::Receiver<Completion>,
    thread: std::thread::JoinHandle<()>,
}
fn destinations() -> &'static Mutex<HashSet<PathBuf>> {
    static PATHS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    PATHS.get_or_init(Mutex::default)
}
struct Destination(PathBuf);
impl Drop for Destination {
    fn drop(&mut self) {
        if let Ok(mut paths) = destinations().lock() {
            paths.remove(&self.0);
        }
    }
}
struct State {
    destination: Option<Destination>,
    storage: Arc<DesktopStorage>,
    scene: Scene,
    sources: HashMap<BlobId, Handle>,
    journal: Option<File>,
    blobs: HashSet<BlobId>,
    undo: VecDeque<Command>,
    redo: VecDeque<Command>,
    poisoned: bool,
}
fn error(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}
pub(super) fn sync_directory(path: &std::path::Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    let _ = path;
    Ok(())
}
fn private_file(path: &std::path::Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).append(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
fn create(path: &std::path::Path) -> io::Result<File> {
    let mut file = private_file(path)?;
    board::initialize(&mut file).map_err(error)?;
    file.sync_all()?;
    sync_directory(super::parent_directory(path))?;
    Ok(file)
}
fn record(file: &mut File, kind: u8, bytes: &[u8]) -> io::Result<()> {
    let offset = file.metadata()?.len();
    if offset.saturating_add(bytes.len() as u64 + 96) > 8 * 1024 * 1024 * 1024 {
        return Err(error(
            "Board journal budget full (8 GiB); save to a new board",
        ));
    }
    board::append(file, offset, kind, bytes).map_err(error)?;
    file.flush()?;
    file.sync_all()
}
impl State {
    fn claim(&mut self, path: &std::path::Path, opening: bool) -> io::Result<()> {
        let canonical = if path.exists() {
            path.canonicalize()?
        } else {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            parent.canonicalize()?.join(
                path.file_name()
                    .ok_or_else(|| error("Board filename is required"))?,
            )
        };
        if self.destination.as_ref().is_some_and(|d| d.0 == canonical) {
            return Ok(());
        }
        if !opening && path.exists() {
            return Err(error(
                "This board already exists. Open it before saving, or choose a new filename.",
            ));
        }
        {
            let mut paths = destinations()
                .lock()
                .map_err(|_| error("Board destination registry unavailable"))?;
            if !paths.insert(canonical.clone()) {
                return Err(error("This board is already open in another tab."));
            }
        }
        self.destination = Some(Destination(canonical));
        Ok(())
    }

    fn journal(&mut self) -> io::Result<&mut File> {
        if self.journal.is_none() {
            let directory = self.storage.recovery_directory()?;
            std::fs::create_dir_all(&directory)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
            }
            self.journal = Some(create(&directory.join("board.moth"))?);
            sync_directory(&directory)?;
            sync_directory(&self.storage.base_directory()?)?;
        }
        self.journal
            .as_mut()
            .ok_or_else(|| error("journal unavailable"))
    }
    fn commit(&mut self, kind: u8, bytes: &[u8]) -> io::Result<()> {
        let result = self.journal().and_then(|file| record(file, kind, bytes));
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn original(&self, hash: BlobId, handle: Handle) -> io::Result<Vec<u8>> {
        let mut source = self.storage.open(handle)?;
        let size = source.seek(SeekFrom::End(0))?;
        if size > board::MAX_RECORD {
            return Err(error("Original exceeds record budget"));
        }
        source.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        source.take(board::MAX_RECORD + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > board::MAX_RECORD || blake3::hash(&bytes) != hash {
            return Err(error("Source changed before it could be saved"));
        }
        Ok(bytes)
    }
    fn apply(&mut self, command: Command, source: Option<Handle>) -> io::Result<Completion> {
        // Validate before committing; metadata-only inverse rollback cannot lose pixels.
        let inverse = self.scene.apply(command.clone()).map_err(error)?;
        self.scene.apply(inverse.clone()).map_err(error)?;
        if let Command::Insert(item) = &command
            && !self.blobs.contains(&item.blob)
        {
            let handle = source
                .or_else(|| self.sources.get(&item.blob).copied())
                .ok_or_else(|| error("Original unavailable"))?;
            let bytes = self.original(item.blob, handle)?;
            self.journal()?;
            let directory = self.storage.recovery_directory()?;
            let path = directory.join(format!("{}.source", uuid::Uuid::now_v7()));
            let mut staged = private_file(&path)?;
            staged.write_all(&bytes)?;
            staged.sync_all()?;
            sync_directory(&directory)?;
            let staged = self.storage.register(path)?;
            self.commit(1, &bytes)?;
            self.blobs.insert(item.blob);
            self.sources.insert(item.blob, staged);
        }
        self.commit(2, &board::encode(&command))?;
        self.scene.apply(command.clone()).map_err(error)?;
        self.undo.push_back(inverse);
        if self.undo.len() > 256 {
            self.undo.pop_front();
        }
        let source = match &command {
            Command::Insert(item) => self.sources.get(&item.blob).map(|h| (item.blob, *h)),
            _ => None,
        };
        Ok(Completion::Applied(command, source))
    }
    fn save(&mut self, path: PathBuf) -> io::Result<Completion> {
        self.claim(&path, false)?;
        let mut file = match OpenOptions::new().read(true).append(true).open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => create(&path)?,
            Err(e) => return Err(e),
        };
        let previous = board::read(&mut file).map_err(error)?;
        // Backups are immutable, uniquely named recovery files; keep the latest three.
        if !previous.scene.is_empty() {
            let directory = self.storage.base_directory()?.join("backups").join(
                blake3::hash(path.as_os_str().as_encoded_bytes())
                    .to_hex()
                    .as_str(),
            );
            std::fs::create_dir_all(&directory)?;
            let backup = directory.join(format!("{}.moth", uuid::Uuid::now_v7()));
            let mut output = private_file(&backup)?;
            file.seek(SeekFrom::Start(0))?;
            io::copy(&mut file, &mut output)?;
            output.sync_all()?;
            sync_directory(&directory)?;
            let mut backups = std::fs::read_dir(&directory)?
                .take(1024)
                .collect::<Result<Vec<_>, _>>()?;
            backups.sort_by_key(|e| e.file_name());
            let excess = backups.len().saturating_sub(3);
            for entry in backups.into_iter().take(excess) {
                std::fs::remove_file(entry.path())?;
            }
        }
        for (hash, handle) in &self.sources {
            if !previous.blobs.contains_key(hash) {
                let bytes = self.original(*hash, *handle)?;
                record(&mut file, 1, &bytes)?;
            }
        }
        record(&mut file, 3, &board::snapshot(&self.scene).map_err(error)?)?;
        let live_bytes: u64 = self
            .scene
            .items()
            .map(|i| previous.blobs.get(&i.blob).map_or(0, |b| b.length))
            .sum();
        if file.metadata()?.len() > (live_bytes.saturating_mul(2)).max(1024 * 1024) {
            drop(file);
            let handle = self.storage.register(path.clone())?;
            let result = board::compact(self.storage.as_ref(), handle).map_err(error);
            self.storage.forget(handle)?;
            result?;
        }
        if let Err(e) = recent(&self.storage.base_directory()?, Some(path)) {
            tracing::warn!(%e, "recent file catalog unavailable");
        }
        Ok(Completion::Saved)
    }
    fn open(&mut self, path: PathBuf) -> io::Result<Completion> {
        self.claim(&path, true)?;
        if !self.scene.is_empty() {
            return Err(error(
                "Open requires an empty board; save and restart first",
            ));
        }
        let mut file = File::open(&path)?;
        let loaded = board::read(&mut file).map_err(error)?;
        let directory = self.storage.recovery_directory()?;
        std::fs::create_dir_all(&directory)?;
        let mut sources = HashMap::new();
        let wanted: HashSet<_> = loaded.scene.items().map(|i| i.blob).collect();
        for hash in wanted {
            let blob = loaded
                .blobs
                .get(&hash)
                .ok_or_else(|| error("Missing original"))?;
            let destination = directory.join(format!("{}.source", uuid::Uuid::now_v7()));
            let mut output = private_file(&destination)?;
            file.seek(SeekFrom::Start(blob.offset))?;
            let copied = io::copy(&mut (&mut file).take(blob.length), &mut output)?;
            if copied != blob.length {
                return Err(error("Truncated original"));
            }
            output.sync_all()?;
            sources.insert(hash, self.storage.register(destination)?);
        }
        sync_directory(&directory)?;
        for (hash, handle) in &sources {
            let bytes = self.original(*hash, *handle)?;
            self.commit(1, &bytes)?;
            self.blobs.insert(*hash);
        }
        self.commit(3, &board::snapshot(&loaded.scene).map_err(error)?)?;
        self.sources = sources.clone();
        self.undo.clear();
        self.redo.clear();
        for item in loaded.scene.items() {
            self.scene
                .apply(Command::Insert(item.clone()))
                .map_err(error)?;
        }
        if let Err(e) = recent(&self.storage.base_directory()?, Some(path)) {
            tracing::warn!(%e, "recent file catalog unavailable");
        }
        Ok(Completion::Opened(loaded.scene, sources, loaded.recovered))
    }
    fn request(&mut self, request: Request) -> io::Result<Completion> {
        if self.poisoned {
            return Err(error(
                "Journal write failed; restart and recover before editing",
            ));
        }
        match request {
            Request::Apply(command, source) => {
                let result = self.apply(command, source)?;
                self.redo.clear();
                Ok(result)
            }
            Request::Undo => {
                let command = self
                    .undo
                    .pop_back()
                    .ok_or_else(|| error("Nothing to undo"))?;
                let result = self.apply(command.clone(), None);
                match result {
                    Ok(done) => {
                        if let Some(inverse) = self.undo.pop_back() {
                            self.redo.push_back(inverse);
                        }
                        Ok(done)
                    }
                    Err(e) => {
                        self.undo.push_back(command);
                        Err(e)
                    }
                }
            }
            Request::Redo => {
                let command = self
                    .redo
                    .pop_back()
                    .ok_or_else(|| error("Nothing to redo"))?;
                match self.apply(command.clone(), None) {
                    Ok(done) => Ok(done),
                    Err(e) => {
                        self.redo.push_back(command);
                        Err(e)
                    }
                }
            }
            Request::Save(path) => self.save(path),
            Request::Open(path) => self.open(path),
        }
    }
}
fn recent(directory: &std::path::Path, path: Option<PathBuf>) -> io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(directory)?;
    let catalog = directory.join("recent.txt");
    let read_catalog = || -> io::Result<String> {
        let mut text = String::new();
        File::open(&catalog)?
            .take(64 * 1024)
            .read_to_string(&mut text)?;
        Ok(text)
    };
    let mut entries: Vec<_> = match read_catalog() {
        Ok(text) => text.lines().take(20).map(PathBuf::from).collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    if let Some(path) = path
        && let Some(text) = path.to_str().filter(|s| !s.contains(['\n', '\r']))
    {
        entries.retain(|p| p != &path);
        entries.insert(0, PathBuf::from(text));
        entries.truncate(20);
        // This is a disposable catalog, never board data.
        std::fs::write(
            catalog,
            entries
                .iter()
                .filter_map(|p| p.to_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )?;
    }
    Ok(entries)
}
impl Worker {
    /// Starts storage and recovery discovery away from the event-loop thread.
    pub fn new(
        storage: Arc<DesktopStorage>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        Self::start(storage, wake, true)
    }
    /// Additional tabs already have the application's recovery and recent-file catalog.
    pub fn new_tab(
        storage: Arc<DesktopStorage>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        Self::start(storage, wake, false)
    }
    fn start(
        storage: Arc<DesktopStorage>,
        wake: Arc<dyn Fn() + Send + Sync>,
        discover: bool,
    ) -> io::Result<Self> {
        let (requests, rx) = crossbeam_channel::bounded(32);
        let (tx, results) = crossbeam_channel::bounded(32);
        let thread = std::thread::Builder::new()
            .name("storage".into())
            .spawn(move || {
                if discover {
                    let recovery = (|| -> io::Result<Vec<PathBuf>> {
                        let directory = storage.base_directory()?;
                        std::fs::create_dir_all(&directory)?;
                        let mut paths = Vec::new();
                        for entry in std::fs::read_dir(directory)?.take(1024) {
                            let entry = entry?;
                            if entry.file_type()?.is_dir() {
                                let path = entry.path().join("board.moth");
                                if path.is_file() {
                                    paths.push(path);
                                }
                            }
                        }
                        Ok(paths)
                    })();
                    let initial = match recovery {
                        Ok(paths) => Completion::Recovery(paths),
                        Err(e) => Completion::Failed(e.to_string()),
                    };
                    if tx.send(initial).is_err() {
                        return;
                    }
                    wake();
                    if let Ok(paths) = storage
                        .base_directory()
                        .and_then(|directory| recent(&directory, None))
                    {
                        if tx.send(Completion::Recent(paths)).is_err() {
                            return;
                        }
                        wake();
                    }
                }
                let mut state = State {
                    destination: None,
                    storage,
                    scene: Scene::default(),
                    sources: HashMap::new(),
                    journal: None,
                    blobs: HashSet::new(),
                    undo: VecDeque::new(),
                    redo: VecDeque::new(),
                    poisoned: false,
                };
                while let Ok(request) = rx.recv() {
                    let result = state
                        .request(request)
                        .unwrap_or_else(|e| Completion::Failed(e.to_string()));
                    if tx.send(result).is_err() {
                        break;
                    }
                    wake();
                }
            })?;
        Ok(Self {
            requests,
            results,
            thread,
        })
    }
    /// Joins only from tools/tests, never from the event-loop thread.
    pub fn finish(self) -> io::Result<()> {
        drop(self.requests);
        drop(self.results);
        self.thread
            .join()
            .map_err(|_| error("storage worker panicked"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tabs_cannot_save_over_an_unopened_or_already_owned_board() {
        let mut first = state();
        let directory = first.storage.base_directory().unwrap();
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("owned.moth");
        first.save(path.clone()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut second = state();
        assert!(second.save(path.clone()).is_err());
        assert!(second.open(path.clone()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        drop(first);
        second.open(path.clone()).unwrap();
        second.save(path.clone()).unwrap();
    }
    fn state() -> State {
        State {
            destination: None,
            storage: Arc::new(DesktopStorage::in_directory(
                std::env::temp_dir().join(format!("mothref-board-test-{}", uuid::Uuid::now_v7())),
            )),
            scene: Scene::default(),
            sources: HashMap::new(),
            journal: None,
            blobs: HashSet::new(),
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            poisoned: false,
        }
    }
    #[test]
    fn journal_undo_redo_save_open_and_compact() {
        let mut state = state();
        let handle = state.storage.stage_rgba(1, 1, &[1, 2, 3, 255]).unwrap();
        let mut bytes = Vec::new();
        state
            .storage
            .open(handle)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        let item = moth_scene::Item::image(
            blake3::hash(&bytes),
            moth_scene::Rect {
                x: 1.,
                y: 2.,
                width: 3.,
                height: 4.,
            },
            None,
        );
        state
            .request(Request::Apply(Command::Insert(item.clone()), Some(handle)))
            .unwrap();
        let journal = state
            .storage
            .recovery_directory()
            .unwrap()
            .join("board.moth");
        assert_eq!(
            board::read(&mut File::open(&journal).unwrap())
                .unwrap()
                .scene
                .get(item.id),
            Some(&item)
        );
        state.request(Request::Undo).unwrap();
        assert!(state.scene.is_empty());
        state.request(Request::Redo).unwrap();
        assert_eq!(state.scene.get(item.id), Some(&item));
        let path = state
            .storage
            .recovery_directory()
            .unwrap()
            .join("saved.moth");
        state.request(Request::Save(path.clone())).unwrap();
        state.request(Request::Undo).unwrap();
        state.request(Request::Save(path.clone())).unwrap();
        assert!(
            board::read(&mut File::open(&path).unwrap())
                .unwrap()
                .scene
                .is_empty()
        );
        state.request(Request::Redo).unwrap();
        state.request(Request::Save(path.clone())).unwrap();
        let old_size = std::fs::metadata(&path).unwrap().len();
        let h = state.storage.register(path.clone()).unwrap();
        board::compact(state.storage.as_ref(), h).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() < old_size);
        // Reopening belongs to a new session after the writer releases the destination.
        drop(state.destination.take());
        let mut loaded = self::state();
        assert!(matches!(
            loaded.request(Request::Open(path.clone())),
            Ok(Completion::Opened(_, _, false))
        ));
        assert_eq!(loaded.scene.get(item.id), Some(&item));
        let backup_directory = state
            .storage
            .base_directory()
            .unwrap()
            .join("backups")
            .join(
                blake3::hash(path.as_os_str().as_encoded_bytes())
                    .to_hex()
                    .as_str(),
            );
        state.journal = None;
        loaded.journal = None;
        state.storage.discard().unwrap();
        assert!(
            backup_directory.is_dir(),
            "explicit close must preserve backups"
        );
        assert!(std::fs::read_dir(&backup_directory).unwrap().count() <= 3);
        std::fs::remove_dir_all(backup_directory).unwrap();
        loaded.storage.discard().unwrap();
        std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
        std::fs::remove_dir_all(loaded.storage.base_directory().unwrap()).unwrap();
    }
    #[test]
    fn failed_journal_write_stops_mutations_and_preserves_last_board() {
        let mut state = state();
        let handle = state.storage.stage_rgba(1, 1, &[4, 5, 6, 255]).unwrap();
        let mut bytes = Vec::new();
        state
            .storage
            .open(handle)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        let item = moth_scene::Item::image(
            blake3::hash(&bytes),
            moth_scene::Rect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            None,
        );
        state
            .request(Request::Apply(Command::Insert(item.clone()), Some(handle)))
            .unwrap();
        let path = state
            .storage
            .recovery_directory()
            .unwrap()
            .join("board.moth");
        state.journal = Some(File::open(&path).unwrap());
        assert!(
            state
                .request(Request::Apply(Command::Remove(item.id), None))
                .is_err()
        );
        assert!(state.poisoned);
        assert_eq!(state.scene.get(item.id), Some(&item));
        assert!(state.request(Request::Undo).is_err());
        assert_eq!(
            board::read(&mut File::open(path).unwrap())
                .unwrap()
                .scene
                .get(item.id),
            Some(&item)
        );
        state.journal = None;
        state.storage.discard().unwrap();
        std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
    }
    #[test]
    fn failed_compaction_replacement_keeps_original_bytes() {
        struct FailReplace<'a>(&'a DesktopStorage);
        impl Storage for FailReplace<'_> {
            fn open(&self, h: Handle) -> io::Result<Box<dyn moth_format::ReadSeek>> {
                self.0.open(h)
            }
            fn append(&self, h: Handle) -> io::Result<Box<dyn Write + Send>> {
                self.0.append(h)
            }
            fn sync(&self, h: Handle) -> io::Result<()> {
                self.0.sync(h)
            }
            fn temporary(&self, h: Handle) -> io::Result<Handle> {
                self.0.temporary(h)
            }
            fn replace(&self, _: Handle, _: Handle) -> io::Result<()> {
                Err(io::Error::other("injected replacement failure"))
            }
        }
        let state = state();
        let directory = state.storage.recovery_directory().unwrap();
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("original.moth");
        let mut file = create(&path).unwrap();
        record(&mut file, 3, &board::snapshot(&Scene::default()).unwrap()).unwrap();
        drop(file);
        let before = std::fs::read(&path).unwrap();
        let h = state.storage.register(path.clone()).unwrap();
        assert!(board::compact(&FailReplace(state.storage.as_ref()), h).is_err());
        assert_eq!(std::fs::read(path).unwrap(), before);
        state.storage.discard().unwrap();
        std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
    }
    #[test]
    fn changed_source_is_not_acknowledged() {
        let mut state = state();
        let handle = state.storage.stage_rgba(1, 1, &[0, 0, 0, 255]).unwrap();
        let item = moth_scene::Item::image(
            blake3::hash(b"wrong"),
            moth_scene::Rect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            None,
        );
        assert!(
            state
                .request(Request::Apply(Command::Insert(item), Some(handle)))
                .is_err()
        );
        assert!(state.scene.is_empty());
        state.storage.discard().unwrap();
        std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
    }
}
