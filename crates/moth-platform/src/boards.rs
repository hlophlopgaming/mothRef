// SPDX-License-Identifier: GPL-3.0-or-later
//! Serializes durable mutations on a bounded storage queue.
use super::DesktopStorage;
mod recovery;
use moth_format::{Handle, Storage, board};
use moth_scene::{BlobId, Command, Item, Rect, Scene};
pub use recovery::Recovery;
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
    /// Applies a whole command list as one undo step (synchronous multi-commands).
    ApplyBatch(Vec<(Command, Option<Handle>)>),
    /// Opens an undo group: following `Apply`s accumulate into one step until `EndBatch`.
    BeginBatch,
    /// Closes the undo group opened by `BeginBatch`.
    EndBatch,
    /// Journal the inverse before acknowledging it.
    Undo,
    /// Journal the previously undone command.
    Redo,
    /// Append an atomic checkpoint, keeping the old checkpoint intact.
    Save(PathBuf),
    /// Load into an empty board and establish a new recovery journal.
    Open(PathBuf),
    /// Restore a validated private journal, retaining its original save destination.
    Restore(PathBuf),
    /// Remove only an explicitly rejected journal in the application recovery folder.
    DiscardRecovery(PathBuf),
    /// Convert a BeeRef `.bee` database into reversible inserts on an empty board.
    ImportBee(PathBuf),
    /// Convert a PureRef `.pur` board through the bounded local purcore fork.
    ImportPur(PathBuf),
    /// Persist the selected interface language code (a two-letter tag).
    SaveSettings(String),
    /// Persist a validated preference change together on the storage thread.
    Preferences(std::collections::BTreeMap<String, String>),
    /// Restore original image dimensions, optionally matching the first image's scale.
    OriginalGeometry {
        /// Stable image identities in reference order.
        ids: Vec<moth_scene::Uuid>,
        /// Original-backed reset or normalization to perform.
        mode: super::export::Geometry,
    },
    /// Export from the worker scene without blocking a frame or changing the board.
    Export(super::export::Target),
}
/// Worker completions are delivered only after durable writes.
pub enum Completion {
    /// Durable loading progress; does not finish the outstanding open/import request.
    Loading {
        /// Commands whose originals and journal records have reached stable storage.
        commands: Vec<Command>,
        /// Newly available journal-backed originals.
        sources: Vec<(BlobId, Handle)>,
        /// Complete board bounds when known before loading the originals.
        bounds: Option<Rect>,
    },
    /// Offers existing journals without silently replacing the current board.
    Recovery(Vec<Recovery>),
    /// The selected private recovery journal has been explicitly discarded.
    RecoveryDiscarded(PathBuf),
    /// Read-only recent-file suggestions, bounded to twenty entries.
    Recent(Vec<PathBuf>),
    /// The persisted interface language, or a detected default on first launch.
    Settings(String),
    /// Saved preferences loaded once at startup.
    Preferences(std::collections::BTreeMap<String, String>),
    /// Confirms a settings write reached the catalog file.
    SettingsSaved,
    /// Export succeeded; sampling/coordinate details are returned when applicable.
    Exported(String),
    /// Applies an already-journaled mutation on the UI thread.
    Applied(Command, Option<(BlobId, Handle)>),
    /// Applies a whole journaled group as one unit.
    AppliedBatch(Vec<Command>, Vec<(BlobId, Handle)>),
    /// Finishes loading recovered metadata and journal-backed originals.
    Opened(Scene, HashMap<BlobId, Handle>, bool, Option<PathBuf>),
    /// Imported inserts, their journal-backed originals and the count of skipped items.
    Imported(Vec<Command>, HashMap<BlobId, Handle>, usize),
    /// Confirms an explicit save has reached stable storage.
    Saved,
    /// Keeps unacknowledged edits hidden; earlier durable loading progress remains available.
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
/// One undo/redo entry: a single command or a group applied as one user action.
///
/// Undo/redo is in-memory only (the journal still stores each command individually),
/// so grouping never changes crash recovery or the `.moth` format.
#[derive(Clone, Debug)]
enum Step {
    Single(Command),
    Group(Vec<Command>),
}
struct Progress {
    results: crossbeam_channel::Sender<Completion>,
    wake: Arc<dyn Fn() + Send + Sync>,
}
#[derive(Default)]
struct LoadBatch {
    commands: Vec<Command>,
    sources: Vec<(BlobId, Handle)>,
    bytes: usize,
    started: bool,
    bounds: Option<Rect>,
}
struct State {
    progress: Option<Progress>,
    destination: Option<Destination>,
    storage: Arc<DesktopStorage>,
    scene: Scene,
    sources: HashMap<BlobId, Handle>,
    journal: Option<File>,
    journal_guard: Option<File>,
    blobs: HashSet<BlobId>,
    undo: VecDeque<Step>,
    redo: VecDeque<Step>,
    /// Inverses accumulated by an open `BeginBatch` group, when one is open.
    batch: Option<Vec<Command>>,
    poisoned: bool,
    clipboard: Option<arboard::Clipboard>,
    undo_limit: usize,
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
fn append_record(file: &mut File, kind: u8, bytes: &[u8]) -> io::Result<()> {
    let offset = file.metadata()?.len();
    if offset.saturating_add(bytes.len() as u64 + 96) > 8 * 1024 * 1024 * 1024 {
        return Err(error(
            "Board journal budget full (8 GiB); save to a new board",
        ));
    }
    board::append(file, offset, kind, bytes).map_err(error)?;
    Ok(())
}
fn record(file: &mut File, kind: u8, bytes: &[u8]) -> io::Result<()> {
    let _span = tracing::info_span!("durable_record", kind, bytes = bytes.len()).entered();
    append_record(file, kind, bytes)?;
    file.flush()?;
    file.sync_all()
}
impl State {
    fn publish_loading(&mut self, batch: &mut LoadBatch, force: bool) -> io::Result<()> {
        if batch.commands.is_empty()
            || (!force
                && batch.started
                && batch.commands.len() < 64
                && batch.bytes < 32 * 1024 * 1024)
        {
            return Ok(());
        }
        let result = self.journal().and_then(|file| file.sync_all());
        if result.is_err() {
            self.poisoned = true;
        }
        result?;
        let completion = Completion::Loading {
            commands: std::mem::take(&mut batch.commands),
            sources: std::mem::take(&mut batch.sources),
            bounds: batch.bounds.take(),
        };
        batch.started = true;
        batch.bytes = 0;
        if let Some(progress) = &self.progress {
            progress.results.send(completion).map_err(error)?;
            (progress.wake)();
        }
        Ok(())
    }

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
            let guard = recovery::lock(&directory.join("board.moth"))?;
            self.journal = Some(create(&directory.join("board.moth"))?);
            self.journal_guard = Some(guard);
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
    // Only command/checkpoint or loading-batch syncs can acknowledge pending records.
    fn append_pending(&mut self, kind: u8, bytes: &[u8]) -> io::Result<()> {
        let result = self
            .journal()
            .and_then(|file| append_record(file, kind, bytes));
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    // Journal ranges stay valid: saves/compaction replace board files, never this journal.
    fn append_original(&mut self, bytes: &[u8]) -> io::Result<Handle> {
        let offset = self.journal()?.metadata()?.len();
        self.append_pending(1, bytes)?;
        self.storage.register_range(
            self.storage.recovery_directory()?.join("board.moth"),
            board::Blob {
                offset,
                length: bytes.len() as u64,
            },
        )
    }
    fn original(&self, hash: BlobId, handle: Handle) -> io::Result<Vec<u8>> {
        let _span = tracing::info_span!("read_original").entered();
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
    /// Journals and applies one command, returning its inverse and the UI completion.
    /// Does not touch the undo/redo stacks: callers decide how to group the inverse.
    fn apply(
        &mut self,
        command: Command,
        source: Option<Handle>,
    ) -> io::Result<(Command, Completion)> {
        self.apply_command(command, source, true)
    }
    fn apply_command(
        &mut self,
        command: Command,
        source: Option<Handle>,
        sync: bool,
    ) -> io::Result<(Command, Completion)> {
        // Validate before committing; metadata-only inverse rollback cannot lose pixels.
        let inverse = self.scene.apply(command.clone()).map_err(error)?;
        self.scene.apply(inverse.clone()).map_err(error)?;
        if let Command::Insert(item) = &command
            && let Some(blob) = item.blob()
            && !self.blobs.contains(&blob)
        {
            let handle = source
                .or_else(|| self.sources.get(&blob).copied())
                .ok_or_else(|| error("Original unavailable"))?;
            let bytes = self.original(blob, handle)?;
            let staged = self.append_original(&bytes)?;
            self.blobs.insert(blob);
            self.sources.insert(blob, staged);
        }
        let bytes = board::encode(&command);
        if sync {
            self.commit(2, &bytes)?;
        } else {
            self.append_pending(2, &bytes)?;
        }
        self.scene.apply(command.clone()).map_err(error)?;
        let source = match &command {
            Command::Insert(item) => item
                .blob()
                .and_then(|blob| self.sources.get(&blob).map(|h| (blob, *h))),
            _ => None,
        };
        Ok((inverse, Completion::Applied(command, source)))
    }
    /// Records one undo step, trimming to the configured in-memory action budget.
    fn push_undo(&mut self, step: Step) {
        self.undo.push_back(step);
        if self.undo.len() > self.undo_limit {
            self.undo.pop_front();
        }
    }
    fn save(&mut self, path: PathBuf) -> io::Result<Completion> {
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("moth"))
        {
            return Err(error("Working boards must be saved as .moth"));
        }
        self.claim(&path, false)?;
        let mut file = match OpenOptions::new().read(true).append(true).open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => create(&path)?,
            Err(e) => return Err(e),
        };
        file.seek(SeekFrom::Start(0))?;
        let mut header = [0u8; 16];
        file.read_exact(&mut header)?;
        if header[4] < board::SCHEMA {
            drop(file);
            let handle = self.storage.register(path.clone())?;
            let result = board::compact(self.storage.as_ref(), handle).map_err(error);
            self.storage.forget(handle)?;
            result?;
            file = OpenOptions::new().read(true).append(true).open(&path)?;
        }
        file.seek(SeekFrom::Start(0))?;
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
            .filter_map(|i| i.blob())
            .map(|blob| previous.blobs.get(&blob).map_or(0, |b| b.length))
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
        self.journal()?;
        if let Some(destination) = &self.destination {
            recovery::remember(&self.storage, &destination.0)?;
        }
        Ok(Completion::Saved)
    }
    fn open(&mut self, path: PathBuf) -> io::Result<Completion> {
        self.open_from(path.clone(), Some(path), false)
    }
    fn open_from(
        &mut self,
        path: PathBuf,
        destination: Option<PathBuf>,
        restored: bool,
    ) -> io::Result<Completion> {
        let _span = tracing::info_span!("open_board").entered();
        if !self.scene.is_empty() {
            return Err(error(
                "Open requires an empty board; save and restart first",
            ));
        }
        if let Some(destination) = destination {
            if let Err(error) = self.claim(&destination, true) {
                if !restored {
                    return Err(error);
                }
                tracing::warn!(%error, "original destination unavailable; restoring as an unnamed board");
                self.destination = None;
            }
        } else {
            self.destination = None;
        }
        let mut file = File::open(&path)?;
        if file.metadata()?.len() > 8 * 1024 * 1024 * 1024 {
            return Err(error("MothRef board exceeds 8 GiB file limit"));
        }
        let loaded = {
            let _span = tracing::info_span!("parse_moth").entered();
            board::read(&mut file).map_err(error)?
        };
        self.journal()?;
        if let Some(destination) = &self.destination {
            recovery::remember(&self.storage, &destination.0)?;
        }
        let mut batch = LoadBatch {
            bounds: loaded.scene.bounds(),
            ..LoadBatch::default()
        };
        // Groups first, parents before their members, so no insert names a group
        // that does not exist yet; then everything else in file order, which keeps
        // reading the originals sequential.
        let parents_first = loaded.scene.items_parent_first();
        let mut items: Vec<_> = parents_first
            .iter()
            .copied()
            .filter(|i| i.is_group())
            .collect();
        let mut rest: Vec<_> = parents_first
            .into_iter()
            .filter(|i| !i.is_group())
            .collect();
        rest.sort_by_key(|item| {
            item.blob()
                .and_then(|hash| loaded.blobs.get(&hash))
                .map_or(u64::MAX, |blob| blob.offset)
        });
        items.extend(rest);
        let mut copied = HashSet::new();
        let loading = (|| -> io::Result<()> {
            for item in items {
                if let Some(hash) = item.blob()
                    && !copied.contains(&hash)
                {
                    let blob = loaded
                        .blobs
                        .get(&hash)
                        .ok_or_else(|| error("Missing original"))?;
                    let _span =
                        tracing::info_span!("stage_original", bytes = blob.length).entered();
                    file.seek(SeekFrom::Start(blob.offset))?;
                    let mut bytes = vec![0; blob.length as usize];
                    file.read_exact(&mut bytes)?;
                    if blake3::hash(&bytes) != hash {
                        return Err(error("Source changed before it could be saved"));
                    }
                    let handle = self.append_original(&bytes)?;
                    self.blobs.insert(hash);
                    copied.insert(hash);
                    self.sources.insert(hash, handle);
                    batch.sources.push((hash, handle));
                    batch.bytes += bytes.len();
                }
                let command = Command::Insert(item.clone());
                if self.scene.is_empty() {
                    self.undo.clear();
                    self.redo.clear();
                }
                self.append_pending(2, &board::encode(&command))?;
                self.scene.apply(command.clone()).map_err(error)?;
                batch.commands.push(command);
                self.publish_loading(&mut batch, false)?;
            }
            Ok(())
        })();
        if !self.poisoned {
            self.publish_loading(&mut batch, true)?;
        }
        loading?;
        self.commit(3, &board::snapshot(&loaded.scene).map_err(error)?)?;
        self.sources.retain(|hash, _| copied.contains(hash));
        self.blobs = copied;
        self.undo.clear();
        self.redo.clear();
        if let Some(destination) = &self.destination
            && let Err(e) = recent(&self.storage.base_directory()?, Some(destination.0.clone()))
        {
            tracing::warn!(%e, "recent file catalog unavailable");
        }
        Ok(Completion::Opened(
            loaded.scene,
            self.sources.clone(),
            loaded.recovered || restored,
            self.destination
                .as_ref()
                .map(|destination| destination.0.clone()),
        ))
    }
    /// Converts a foreign board into inserts on the current (empty) board.
    ///
    /// Embedded originals are journaled one at a time, so a crash
    /// mid-import recovers every image already acknowledged.
    fn import_foreign(&mut self, path: PathBuf) -> io::Result<Completion> {
        let _span = tracing::info_span!("import_board").entered();
        if !self.scene.is_empty() {
            return Err(error("Import requires an empty board"));
        }
        struct Pending {
            z: f64,
            id: i64,
            item: Item,
        }
        let mut pending = Vec::new();
        let mut metadata_bytes = 0usize;
        let mut batch = LoadBatch::default();
        let mut emit = |source: moth_import::pur::Item<'_>| -> io::Result<()> {
            metadata_bytes += source.text.map_or(0, str::len) + 256;
            if pending.len() >= 8192 || metadata_bytes > 8 * 1024 * 1024 {
                return Err(error("Import metadata exceeds 8 MiB / 8192-item budget"));
            }
            if !source.z.is_finite() || !source.rect.valid() || !source.transform.valid() {
                return Err(error("Invalid imported transform or stacking order"));
            }
            let mut item = if let Some(bytes) = source.bytes {
                let hash = blake3::hash(bytes);
                if !self.blobs.contains(&hash) {
                    let handle = self.append_original(bytes)?;
                    self.sources.insert(hash, handle);
                    self.blobs.insert(hash);
                    batch.sources.push((hash, handle));
                    batch.bytes += bytes.len();
                }
                Item::image(hash, source.rect, self.scene.top())
            } else {
                Item::text(source.text.unwrap_or(""), source.rect, self.scene.top())
            };
            item.transform = source.transform;
            let command = Command::Insert(item.clone());
            let (inverse, _) = self.apply_command(command.clone(), None, false)?;
            if pending.is_empty() {
                self.undo.clear();
                self.undo.push_back(Step::Group(Vec::new()));
                self.redo.clear();
            }
            if let Some(Step::Group(inverses)) = self.undo.back_mut() {
                inverses.push(inverse);
            }
            pending.push(Pending {
                z: source.z,
                id: source.id,
                item,
            });
            batch.commands.push(command);
            self.publish_loading(&mut batch, false)
        };
        let imported = (|| -> io::Result<usize> {
            let file = File::open(&path)?;
            let skipped = if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pur"))
            {
                moth_import::pur::read(file, &mut |item| {
                    emit(item)?;
                    Ok(())
                })
                .map_err(error)?
                .skipped()
            } else {
                moth_import::bee::read(file, &mut |item| {
                    match item {
                        moth_import::bee::Item::Image(image) => emit(moth_import::pur::Item {
                            id: image.id,
                            z: image.z as f64,
                            rect: Rect {
                                x: image.x,
                                y: image.y,
                                width: image.width,
                                height: image.height,
                            },
                            transform: moth_scene::Transform {
                                rotation: image.rotation,
                                flip: [image.flip, false],
                                ..Default::default()
                            },
                            bytes: Some(image.bytes),
                            text: None,
                        })?,
                        moth_import::bee::Item::Text(note) => {
                            let [width, height] = moth_scene::note_size(&note.text);
                            emit(moth_import::pur::Item {
                                id: note.id,
                                z: note.z as f64,
                                rect: Rect {
                                    x: note.x,
                                    y: note.y,
                                    width: width * note.scale,
                                    height: height * note.scale,
                                },
                                transform: moth_scene::Transform {
                                    rotation: note.rotation,
                                    flip: [note.flip, false],
                                    ..Default::default()
                                },
                                bytes: None,
                                text: Some(&note.text),
                            })?;
                        }
                    }
                    Ok(())
                })
                .map_err(error)?
                .skipped()
            };
            Ok(skipped)
        })();
        if !self.poisoned {
            self.publish_loading(&mut batch, true)?;
        }
        let skipped = imported?;
        // Loading uses stable ids in stream order; finish with the foreign stacking order.
        pending.sort_by(|a, b| {
            a.z.partial_cmp(&b.z)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.id.cmp(&b.id))
        });
        let mut commands = Vec::with_capacity(pending.len());
        let mut above = None;
        for entry in pending {
            let item = entry.item;
            let ordered = Item {
                z: above
                    .as_ref()
                    .map(moth_scene::FractionalIndex::new_after)
                    .unwrap_or_default(),
                ..item.clone()
            };
            above = Some(ordered.z.clone());
            if ordered.z != item.z {
                self.apply_command(Command::Remove(item.id), None, false)?;
                self.apply_command(Command::Insert(ordered.clone()), None, false)?;
            }
            commands.push(Command::Insert(ordered));
        }
        self.commit(3, &board::snapshot(&self.scene).map_err(error)?)?;
        if commands.is_empty() {
            self.undo.clear();
        }
        Ok(Completion::Imported(
            commands,
            self.sources.clone(),
            skipped,
        ))
    }
    fn request(&mut self, request: Request) -> io::Result<Option<Completion>> {
        if self.poisoned {
            return Err(error(
                "Journal write failed; restart and recover before editing",
            ));
        }
        match request {
            Request::Apply(command, source) => {
                let (inverse, done) = self.apply(command, source)?;
                match &mut self.batch {
                    Some(group) => group.push(inverse),
                    None => self.push_undo(Step::Single(inverse)),
                }
                self.redo.clear();
                Ok(Some(done))
            }
            Request::ApplyBatch(batch) => {
                let mut inverses = Vec::with_capacity(batch.len());
                let mut commands = Vec::with_capacity(batch.len());
                let mut sources = Vec::new();
                for (command, source) in batch {
                    let (inverse, done) = self.apply(command, source)?;
                    inverses.push(inverse);
                    if let Completion::Applied(command, source) = done {
                        commands.push(command);
                        if let Some(s) = source {
                            sources.push(s);
                        }
                    }
                }
                if !inverses.is_empty() {
                    if let Some(group) = &mut self.batch {
                        group.extend(inverses);
                    } else {
                        self.push_undo(Step::Group(inverses));
                    }
                }
                self.redo.clear();
                Ok(Some(Completion::AppliedBatch(commands, sources)))
            }
            Request::BeginBatch => {
                self.batch.get_or_insert_with(Vec::new);
                Ok(None)
            }
            Request::EndBatch => {
                if let Some(group) = self.batch.take()
                    && !group.is_empty()
                {
                    self.push_undo(Step::Group(group));
                }
                Ok(None)
            }
            Request::Undo => {
                let step = self
                    .undo
                    .pop_back()
                    .ok_or_else(|| error("Nothing to undo"))?;
                Ok(Some(self.replay_undo(step)?))
            }
            Request::Redo => {
                let step = self
                    .redo
                    .pop_back()
                    .ok_or_else(|| error("Nothing to redo"))?;
                Ok(Some(self.replay_redo(step)?))
            }
            Request::Export(target) => {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    super::export::run(
                        &self.storage,
                        &self.scene,
                        &self.sources,
                        target,
                        &mut self.clipboard,
                    )
                }))
                .map_err(|_| error("Export decoder panicked"))??;
                Ok(Some(Completion::Exported(result)))
            }
            Request::Save(path) => Ok(Some(self.save(path)?)),
            Request::Open(path) => Ok(Some(self.open(path)?)),
            Request::Restore(path) => {
                let directory = recovery::validate(&self.storage, &path)?;
                if directory == self.storage.recovery_directory()? {
                    return Err(error("Cannot restore the active journal"));
                }
                let _guard = recovery::lock(&path)?;
                let destination = recovery::destination(&path).unwrap_or_else(|error| {
                    tracing::warn!(%error, "recovery destination unavailable; restoring as an unnamed board");
                    None
                });
                let completion = self.open_from(path.clone(), destination, true)?;
                // Keep the source bytes even when a damaged tail was only partially recoverable.
                let archive = directory.join(format!("restored-{}.moth", uuid::Uuid::now_v7()));
                if let Err(error) =
                    std::fs::rename(path, archive).and_then(|()| sync_directory(&directory))
                {
                    tracing::warn!(%error, "restored recovery journal archival failed");
                }
                Ok(Some(completion))
            }
            Request::DiscardRecovery(path) => {
                let directory = recovery::validate(&self.storage, &path)?;
                if directory == self.storage.recovery_directory()? {
                    return Err(error("Cannot discard the active journal"));
                }
                let _guard = recovery::lock(&path)?;
                std::fs::remove_dir_all(directory)?;
                Ok(Some(Completion::RecoveryDiscarded(path)))
            }
            Request::ImportBee(path) | Request::ImportPur(path) => {
                let result = self.import_foreign(path)?;
                self.redo.clear();
                Ok(Some(result))
            }
            Request::OriginalGeometry { ids, mode } => {
                let commands = super::export::image_geometry(
                    &self.storage,
                    &self.scene,
                    &self.sources,
                    &ids,
                    mode,
                )?;
                self.request(Request::ApplyBatch(
                    commands.into_iter().map(|c| (c, None)).collect(),
                ))
            }
            Request::Preferences(values) => {
                super::settings::save_all(&self.storage.base_directory()?, &values)?;
                if let Some(value) = values.get("undo_limit") {
                    self.undo_limit = value
                        .parse::<usize>()
                        .ok()
                        .filter(|n| *n <= 4096)
                        .unwrap_or(256);
                    self.undo
                        .drain(..self.undo.len().saturating_sub(self.undo_limit));
                    self.redo
                        .drain(..self.redo.len().saturating_sub(self.undo_limit));
                }
                Ok(Some(Completion::SettingsSaved))
            }
            Request::SaveSettings(code) => {
                let directory = self.storage.base_directory()?;
                super::settings::save(&directory, "language", &code)?;
                Ok(Some(Completion::SettingsSaved))
            }
        }
    }
    /// Re-applies a popped undo step, producing the matching redo step.
    fn replay_undo(&mut self, step: Step) -> io::Result<Completion> {
        match step {
            Step::Single(command) => {
                let (inverse, done) = self.apply(command, None)?;
                self.redo.push_back(Step::Single(inverse));
                Ok(done)
            }
            Step::Group(commands) => {
                let mut inverses = Vec::with_capacity(commands.len());
                let mut applied = Vec::with_capacity(commands.len());
                let mut sources = Vec::new();
                for command in commands.into_iter().rev() {
                    let (inverse, done) = self.apply(command, None)?;
                    inverses.push(inverse);
                    if let Completion::Applied(command, source) = done {
                        applied.push(command);
                        if let Some(s) = source {
                            sources.push(s);
                        }
                    }
                }
                self.redo.push_back(Step::Group(inverses));
                Ok(Completion::AppliedBatch(applied, sources))
            }
        }
    }
    /// Re-applies a popped redo step, producing the matching undo step.
    fn replay_redo(&mut self, step: Step) -> io::Result<Completion> {
        match step {
            Step::Single(command) => {
                let (inverse, done) = self.apply(command, None)?;
                self.push_undo(Step::Single(inverse));
                Ok(done)
            }
            Step::Group(commands) => {
                let mut inverses = Vec::with_capacity(commands.len());
                let mut applied = Vec::with_capacity(commands.len());
                let mut sources = Vec::new();
                for command in commands.into_iter().rev() {
                    let (inverse, done) = self.apply(command, None)?;
                    inverses.push(inverse);
                    if let Completion::Applied(command, source) = done {
                        applied.push(command);
                        if let Some(s) = source {
                            sources.push(s);
                        }
                    }
                }
                self.push_undo(Step::Group(inverses));
                Ok(Completion::AppliedBatch(applied, sources))
            }
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
                    let recovery = recovery::discover(&storage);
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
                    if let Ok(preferences) = storage
                        .base_directory()
                        .and_then(|d| super::settings::load(&d))
                        && tx.send(Completion::Preferences(preferences)).is_err()
                    {
                        return;
                    }
                    let language = storage
                        .base_directory()
                        .and_then(|directory| super::settings::load(&directory))
                        .map(|settings| settings.get("language").cloned())
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| super::locale::detect().to_owned());
                    if tx.send(Completion::Settings(language)).is_err() {
                        return;
                    }
                    wake();
                }
                let mut state = State {
                    progress: Some(Progress {
                        results: tx.clone(),
                        wake: wake.clone(),
                    }),
                    destination: None,
                    storage,
                    scene: Scene::default(),
                    sources: HashMap::new(),
                    journal: None,
                    journal_guard: None,
                    blobs: HashSet::new(),
                    undo: VecDeque::new(),
                    redo: VecDeque::new(),
                    batch: None,
                    poisoned: false,
                    clipboard: None,
                    undo_limit: 256,
                };
                state.undo_limit = state
                    .storage
                    .base_directory()
                    .and_then(|d| super::settings::load(&d))
                    .ok()
                    .and_then(|m| m.get("undo_limit").and_then(|s| s.parse::<usize>().ok()))
                    .filter(|n| *n <= 4096)
                    .unwrap_or(256);
                while let Ok(request) = rx.recv() {
                    match state.request(request) {
                        Ok(Some(result)) => {
                            if tx.send(result).is_err() {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            if tx.send(Completion::Failed(e.to_string())).is_err() {
                                break;
                            }
                        }
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
    fn restore_keeps_original_destination_and_preserves_source_until_durable() {
        let mut original = state();
        let root = original.storage.base_directory().unwrap();
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        let directory = {
            let alias = root.join("save-alias");
            std::os::unix::fs::symlink(&root, &alias).unwrap();
            alias
        };
        #[cfg(not(unix))]
        let directory = root.clone();
        let path = directory.join("Artist references.moth");
        original.save(path.clone()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let note = Item::text(
            "unsaved note",
            Rect {
                x: 0.,
                y: 0.,
                width: 120.,
                height: 40.,
            },
            None,
        );
        original
            .request(Request::Apply(Command::Insert(note.clone()), None))
            .unwrap();
        let journal = original
            .storage
            .recovery_directory()
            .unwrap()
            .join("board.moth");
        let mut restored = state();
        restored.storage = Arc::new(DesktopStorage::in_directory(root.clone()));
        assert!(
            restored.request(Request::Restore(journal.clone())).is_err(),
            "live journals must remain untouched"
        );
        assert!(journal.is_file());
        drop(original);
        let completion = restored.request(Request::Restore(journal.clone())).unwrap();
        let canonical = path.canonicalize().unwrap();
        assert!(
            matches!(completion, Some(Completion::Opened(_, _, true, Some(ref destination))) if destination == &canonical)
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "restore must not write the original file"
        );
        assert_eq!(restored.scene.get(note.id), Some(&note));
        assert!(
            !journal.exists(),
            "restored journals must not be offered again"
        );
        assert!(
            std::fs::read_dir(journal.parent().unwrap())
                .unwrap()
                .any(|entry| entry
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "moth")),
            "the original journal bytes stay archived"
        );
        restored.save(path.clone()).unwrap();
        let loaded = board::read(&mut File::open(&path).unwrap()).unwrap();
        assert_eq!(loaded.scene.get(note.id), Some(&note));
        drop(restored);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_recovery_metadata_restores_unnamed_without_touching_the_original() {
        let mut original = state();
        let root = original.storage.base_directory().unwrap();
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("original.moth");
        original.save(path.clone()).unwrap();
        let journal = original
            .storage
            .recovery_directory()
            .unwrap()
            .join("board.moth");
        let bytes = std::fs::read(&journal).unwrap();
        drop(original);
        std::fs::write(
            journal.parent().unwrap().join("destination"),
            b"torn metadata",
        )
        .unwrap();
        let mut restored = state();
        restored.storage = Arc::new(DesktopStorage::in_directory(root.clone()));
        assert!(
            restored
                .request(Request::DiscardRecovery(path.clone()))
                .is_err()
        );
        assert!(path.is_file());
        assert!(matches!(
            restored.request(Request::Restore(journal.clone())).unwrap(),
            Some(Completion::Opened(_, _, true, None))
        ));
        let archived = std::fs::read_dir(journal.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "moth")
            })
            .unwrap();
        assert_eq!(std::fs::read(archived).unwrap(), bytes);
        assert!(path.is_file());
        let live = restored
            .storage
            .recovery_directory()
            .unwrap()
            .join("board.moth");
        assert!(
            restored
                .request(Request::DiscardRecovery(live.clone()))
                .is_err()
        );
        assert!(live.is_file());
        drop(restored);
        std::fs::remove_dir_all(root).unwrap();
    }
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
            progress: None,
            destination: None,
            storage: Arc::new(DesktopStorage::in_directory(
                std::env::temp_dir().join(format!("mothref-board-test-{}", uuid::Uuid::now_v7())),
            )),
            scene: Scene::default(),
            sources: HashMap::new(),
            journal: None,
            journal_guard: None,
            blobs: HashSet::new(),
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            batch: None,
            poisoned: false,
            clipboard: None,
            undo_limit: 256,
        }
    }
    #[test]
    fn opening_inserts_groups_before_their_members() {
        let mut state = state();
        let path = state.storage.base_directory().unwrap().join("groups.moth");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let bytes = include_bytes!("../../../corpus/moth/v6.moth");
        std::fs::write(&path, bytes).unwrap();
        let expected = board::read(&mut std::io::Cursor::new(&bytes[..])).unwrap();
        assert!(expected.scene.items().any(|i| i.parent.is_some()));
        state.request(Request::Open(path)).unwrap();
        assert_eq!(state.scene.len(), expected.scene.len());
        for item in expected.scene.items() {
            assert_eq!(state.scene.get(item.id).unwrap().parent, item.parent);
        }
        state.journal = None;
        state.journal_guard = None;
        state.storage.discard().unwrap();
    }
    #[test]
    fn saving_schema_three_atomically_upgrades_appearance() {
        let mut state = state();
        let path = state.storage.base_directory().unwrap().join("old.moth");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, include_bytes!("../../../corpus/moth/v3.moth")).unwrap();
        state.request(Request::Open(path.clone())).unwrap();
        let original = state.scene.items().next().unwrap().clone();
        let transform = moth_scene::Transform {
            opacity: 0.25,
            grayscale: true,
            ..original.transform
        };
        state
            .request(Request::Apply(
                Command::SetTransform {
                    id: original.id,
                    transform,
                },
                None,
            ))
            .unwrap();
        state.request(Request::Save(path.clone())).unwrap();
        assert_eq!(std::fs::read(&path).unwrap()[4], board::SCHEMA);
        let loaded = board::read(&mut File::open(path).unwrap()).unwrap();
        assert_eq!(loaded.scene.get(original.id).unwrap().transform, transform);
        state.journal = None;
        state.journal_guard = None;
        state.storage.discard().unwrap();
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
            Ok(Some(Completion::Opened(_, _, false, _)))
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
        state.journal_guard = None;
        loaded.journal = None;
        loaded.journal_guard = None;
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
    fn pur_import_recovers_originals_and_undoes_as_one_step() {
        for name in ["v1.10-redbar.pur", "v2.1-redbar.pur"] {
            let mut state = state();
            let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus/pur")
                .join(name);
            assert!(matches!(
                state.request(Request::ImportPur(fixture)).unwrap(),
                Some(Completion::Imported(_, _, 0))
            ));
            let before = board::snapshot(&state.scene).unwrap();
            let journal = state
                .storage
                .recovery_directory()
                .unwrap()
                .join("board.moth");
            let recovered = board::read(&mut File::open(&journal).unwrap()).unwrap();
            assert_eq!(board::snapshot(&recovered.scene).unwrap(), before);
            assert_eq!(recovered.blobs.len(), 1);
            let image = state.scene.items().next().unwrap();
            let hash = image.blob().unwrap();
            let mut original = Vec::new();
            state
                .storage
                .open(state.sources[&hash])
                .unwrap()
                .read_to_end(&mut original)
                .unwrap();
            assert_eq!(original.len(), 157);
            assert_eq!(blake3::hash(&original), hash);
            assert_eq!(state.undo.len(), 1);
            state.request(Request::Undo).unwrap();
            assert!(state.scene.is_empty());
            state.request(Request::Redo).unwrap();
            assert_eq!(board::snapshot(&state.scene).unwrap(), before);
            state.journal = None;
            state.journal_guard = None;
            state.storage.discard().unwrap();
            std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
        }
    }

    #[test]
    fn conversion_crash_writer() {
        let Some(root) = std::env::var_os("MOTHREF_CONVERSION_CRASH_ROOT") else {
            return;
        };
        let source = PathBuf::from(std::env::var_os("MOTHREF_CONVERSION_SOURCE").unwrap());
        let mut state = state();
        state.storage = Arc::new(DesktopStorage::in_directory(PathBuf::from(&root)));
        let request = if source.extension().unwrap() == "bee" {
            Request::ImportBee(source)
        } else {
            Request::ImportPur(source)
        };
        state.request(request).unwrap();
        let item = state.scene.items().next().unwrap().clone();
        state
            .request(Request::Apply(
                Command::SetRect {
                    id: item.id,
                    rect: Rect {
                        x: 123.,
                        ..item.rect
                    },
                },
                None,
            ))
            .unwrap();
        std::fs::write(PathBuf::from(root).join("ready"), b"durable").unwrap();
        loop {
            std::thread::park();
        }
    }

    #[test]
    fn conversion_recovers_after_process_is_killed() {
        for fixture in ["bee/tiny.bee", "pur/v2.1-redbar.pur"] {
            let mut restored = state();
            let root = restored.storage.base_directory().unwrap();
            let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus")
                .join(fixture);
            let before = std::fs::read(&source).unwrap();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "boards::tests::conversion_crash_writer",
                    "--nocapture",
                ])
                .env("MOTHREF_CONVERSION_CRASH_ROOT", &root)
                .env("MOTHREF_CONVERSION_SOURCE", &source)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !root.join("ready").exists() && std::time::Instant::now() < deadline {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let ready = root.join("ready").exists();
            let _ = child.kill();
            child.wait().unwrap();
            assert!(
                ready,
                "child must finish durable import before being killed"
            );
            let recovery = recovery::discover(&restored.storage).unwrap();
            assert_eq!(recovery.len(), 1);
            restored
                .request(Request::Restore(recovery[0].journal.clone()))
                .unwrap();
            assert!(restored.destination.is_none());
            assert!(restored.scene.items().any(|item| item.rect.x == 123.));
            for (hash, handle) in &restored.sources {
                assert_eq!(
                    blake3::hash(&restored.original(*hash, *handle).unwrap()),
                    *hash
                );
            }
            restored.save(root.join("recovered.moth")).unwrap();
            assert_eq!(std::fs::read(source).unwrap(), before);
            restored.journal = None;
            restored.journal_guard = None;
            restored.storage.discard().unwrap();
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn board_admission_is_independent_of_image_limits() {
        let mut imported = state();
        let directory = imported.storage.base_directory().unwrap();
        std::fs::create_dir_all(&directory).unwrap();
        let source = directory.join("large.bee");
        std::fs::write(&source, include_bytes!("../../../corpus/bee/tiny.bee")).unwrap();
        let file = OpenOptions::new().write(true).open(&source).unwrap();
        file.set_len(moth_io::MAX_INPUT_BYTES + 4096).unwrap();
        drop(file);
        imported
            .request(Request::ImportBee(source.clone()))
            .unwrap();
        assert!(!imported.scene.is_empty());
        let mut oversized = state();
        std::fs::create_dir_all(oversized.storage.base_directory().unwrap()).unwrap();
        let file = OpenOptions::new().write(true).open(&source).unwrap();
        file.set_len(4 * 1024 * 1024 * 1024 + 1).unwrap();
        drop(file);
        assert!(oversized.request(Request::ImportBee(source)).is_err());
        let source = directory.join("large.moth");
        let file = File::create(&source).unwrap();
        file.set_len(8 * 1024 * 1024 * 1024 + 1).unwrap();
        drop(file);
        assert!(oversized.open(source).is_err());
        for mut state in [oversized, imported] {
            state.journal = None;
            state.journal_guard = None;
            state.storage.discard().unwrap();
            std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
        }
    }

    #[test]
    fn conversion_saves_reopens_and_never_changes_the_source() {
        for fixture in ["bee/tiny.bee", "pur/v2.1-redbar.pur"] {
            let mut imported = state();
            let directory = imported.storage.base_directory().unwrap();
            std::fs::create_dir_all(&directory).unwrap();
            let source = directory.join(std::path::Path::new(fixture).file_name().unwrap());
            let bytes = std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../corpus")
                    .join(fixture),
            )
            .unwrap();
            std::fs::write(&source, &bytes).unwrap();
            let request = if fixture.ends_with("bee") {
                Request::ImportBee(source.clone())
            } else {
                Request::ImportPur(source.clone())
            };
            imported.request(request).unwrap();
            assert!(imported.destination.is_none());
            assert!(imported.save(source.clone()).is_err());
            assert_eq!(std::fs::read(&source).unwrap(), bytes);
            let mut expected: Vec<_> = imported.scene.items().cloned().collect();
            expected.sort_by_key(|item| item.id);
            let saved = directory.join("converted.moth");
            imported.save(saved.clone()).unwrap();
            imported.destination = None;
            let mut reopened = state();
            reopened.open(saved).unwrap();
            let mut actual: Vec<_> = reopened.scene.items().cloned().collect();
            actual.sort_by_key(|item| item.id);
            assert_eq!(actual, expected);
            for (hash, handle) in &reopened.sources {
                assert_eq!(
                    reopened.original(*hash, *handle).unwrap(),
                    imported.original(*hash, imported.sources[hash]).unwrap()
                );
            }
            assert_eq!(std::fs::read(source).unwrap(), bytes);
            for mut state in [reopened, imported] {
                state.journal = None;
                state.journal_guard = None;
                state.storage.discard().unwrap();
                std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
            }
        }
    }

    #[test]
    fn foreign_import_keeps_originals_only_in_journal() {
        for fixture in ["bee/tiny.bee", "pur/v2.1-redbar.pur"] {
            let mut state = state();
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus")
                .join(fixture);
            let request = if fixture.ends_with("bee") {
                Request::ImportBee(path)
            } else {
                Request::ImportPur(path)
            };
            state.request(request).unwrap();
            let directory = state.storage.recovery_directory().unwrap();
            let originals = std::fs::read_dir(&directory)
                .unwrap()
                .map(Result::unwrap)
                .filter(|entry| entry.path().extension().is_some_and(|e| e == "source"))
                .count();
            assert_eq!(
                originals, 0,
                "originals must be read directly from the journal"
            );
            let recovered =
                board::read(&mut File::open(directory.join("board.moth")).unwrap()).unwrap();
            assert_eq!(recovered.scene.len(), state.scene.len());
            assert_eq!(recovered.blobs.len(), state.sources.len());
            state.journal = None;
            state.journal_guard = None;
            // The journal alone must suffice to reopen every original.
            let mut restored = self::state();
            restored
                .request(Request::Open(directory.join("board.moth")))
                .unwrap();
            assert_eq!(restored.scene.len(), state.scene.len());
            for (hash, handle) in &restored.sources {
                let bytes = restored.original(*hash, *handle).unwrap();
                assert_eq!(blake3::hash(&bytes), *hash);
            }
            restored.journal = None;
            restored.journal_guard = None;
            restored.storage.discard().unwrap();
            std::fs::remove_dir_all(restored.storage.base_directory().unwrap()).unwrap();
            state.storage.discard().unwrap();
            std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
        }
    }

    #[test]
    fn opening_reads_originals_from_its_own_journal() {
        let mut state = state();
        let directory = state.storage.base_directory().unwrap();
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("input.moth");
        std::fs::write(&path, include_bytes!("../../../corpus/moth/v4.moth")).unwrap();
        let (results, progress) = crossbeam_channel::bounded(32);
        state.progress = Some(Progress {
            results,
            wake: Arc::new(|| {}),
        });
        state.open(path.clone()).unwrap();
        assert!(
            matches!(progress.try_recv().unwrap(), Completion::Loading { commands, sources, bounds: Some(_) }
            if commands.len() == 1 && sources.len() == 1)
        );
        let items: Vec<_> = state.scene.items().cloned().collect();
        let assert_scene = |scene: &Scene| {
            assert_eq!(scene.len(), items.len());
            for item in &items {
                assert_eq!(scene.get(item.id), Some(item));
            }
        };
        let recovery = state.storage.recovery_directory().unwrap();
        let mut entries: Vec<_> = std::fs::read_dir(&recovery)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        entries.sort();
        assert_eq!(
            entries,
            ["active", "board.moth", "destination"],
            "opening keeps only the journal and its session metadata, without duplicating originals"
        );
        // Reads must survive replacement/compaction of the opened board.
        let handle = state.storage.register(path.clone()).unwrap();
        board::compact(state.storage.as_ref(), handle).unwrap();
        std::fs::remove_file(&path).unwrap();
        for (hash, handle) in &state.sources {
            assert_eq!(
                blake3::hash(&state.original(*hash, *handle).unwrap()),
                *hash
            );
        }
        let image = items.iter().find(|item| item.blob().is_some()).unwrap();
        let exported = directory.join("original.png");
        state
            .request(Request::Export(super::super::export::Target::Original {
                path: exported.clone(),
                id: image.id,
            }))
            .unwrap();
        assert_eq!(
            blake3::hash(&std::fs::read(exported).unwrap()),
            image.blob().unwrap()
        );
        let output = directory.join("saved.moth");
        state.save(output.clone()).unwrap();
        let saved = board::read(&mut File::open(output).unwrap()).unwrap();
        assert_scene(&saved.scene);
        // Recovery itself must also open without any loose original files.
        state.journal = None;
        state.journal_guard = None;
        let mut restored = self::state();
        restored.open(recovery.join("board.moth")).unwrap();
        assert_scene(&restored.scene);
        for (hash, handle) in &restored.sources {
            assert_eq!(
                blake3::hash(&restored.original(*hash, *handle).unwrap()),
                *hash
            );
        }
        restored.journal = None;
        restored.journal_guard = None;
        restored.storage.discard().unwrap();
        std::fs::remove_dir_all(restored.storage.base_directory().unwrap()).unwrap();
        state.storage.discard().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn bee_import_journals_embedded_images_and_notes() {
        let mut state = state();
        let fixture = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/bee/tiny.bee"
        ));
        let (image_rect, note) = match state.request(Request::ImportBee(fixture)).unwrap() {
            Some(Completion::Imported(commands, sources, skipped)) => {
                assert_eq!(commands.len(), 2);
                assert_eq!(sources.len(), 1);
                assert_eq!(skipped, 0);
                let image = state.scene.items().find(|i| i.blob().is_some()).unwrap();
                let note = state
                    .scene
                    .items()
                    .find(|i| i.note().is_some())
                    .map(|i| (i.rect.x, i.rect.y, i.note().unwrap().to_owned()))
                    .unwrap();
                (image.rect, note)
            }
            _ => panic!("expected an import completion"),
        };
        assert_eq!(
            (
                image_rect.x,
                image_rect.y,
                image_rect.width,
                image_rect.height
            ),
            (10., 20., 2., 1.)
        );
        assert_eq!((note.0, note.1), (30., 40.));
        assert_eq!(note.2, "hi");
        // Every embedded original is journaled before it is acknowledged.
        let journal = state
            .storage
            .recovery_directory()
            .unwrap()
            .join("board.moth");
        let recovered = board::read(&mut File::open(journal).unwrap()).unwrap();
        assert_eq!(recovered.scene.len(), 2);
        assert!(recovered.scene.items().any(|i| i.note() == Some("hi")));
        state.journal = None;
        state.journal_guard = None;
        state.storage.discard().unwrap();
        std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
    }

    #[test]
    fn bee_import_applies_rotation_flip_and_scale() {
        let mut state = state();
        let fixture = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/bee/transforms.bee"
        ));
        match state.request(Request::ImportBee(fixture)).unwrap() {
            Some(Completion::Imported(commands, _, skipped)) => {
                assert_eq!(commands.len(), 2);
                assert_eq!(skipped, 0);
            }
            _ => panic!("expected an import completion"),
        }
        let image = state.scene.items().find(|i| i.blob().is_some()).unwrap();
        assert!((image.transform.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert_eq!(image.transform.flip, [true, false]);
        assert_eq!((image.rect.width, image.rect.height), (8.0, 4.0));
        let note = state.scene.items().find(|i| i.note().is_some()).unwrap();
        assert!((note.transform.rotation - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        assert_eq!(note.transform.flip, [true, false]);
        let [w, h] = moth_scene::note_size("note");
        assert_eq!((note.rect.width, note.rect.height), (w * 1.5, h * 1.5));
        state.journal = None;
        state.journal_guard = None;
        state.storage.discard().unwrap();
        std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
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
        state.journal_guard = None;
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
    #[test]
    fn arrangement_batch_stays_inside_the_import_undo_group() {
        let mut state = state();
        let item = Item::text(
            "note",
            Rect {
                x: 0.,
                y: 0.,
                width: 20.,
                height: 20.,
            },
            None,
        );
        let id = item.id;
        state.request(Request::BeginBatch).unwrap();
        state
            .request(Request::Apply(Command::Insert(item), None))
            .unwrap();
        state
            .request(Request::ApplyBatch(vec![(
                Command::SetRect {
                    id,
                    rect: Rect {
                        x: 100.,
                        y: 100.,
                        width: 20.,
                        height: 20.,
                    },
                },
                None,
            )]))
            .unwrap();
        state.request(Request::EndBatch).unwrap();
        assert_eq!(state.undo.len(), 1);
        state.request(Request::Undo).unwrap();
        assert!(state.scene.is_empty());
        state.request(Request::Redo).unwrap();
        assert_eq!(state.scene.get(id).unwrap().rect.x, 100.);
        state.request(Request::Undo).unwrap();
        assert!(state.scene.is_empty());
        state.storage.discard().unwrap();
        std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
    }
    #[test]
    fn batch_group_undoes_and_redoes_as_one_step() {
        let mut state = state();
        let handle = state.storage.stage_rgba(1, 1, &[9, 9, 9, 255]).unwrap();
        let mut bytes = Vec::new();
        state
            .storage
            .open(handle)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        let hash = blake3::hash(&bytes);
        let rect = moth_scene::Rect {
            x: 0.,
            y: 0.,
            width: 1.,
            height: 1.,
        };
        state.request(Request::BeginBatch).unwrap();
        for _ in 0..3 {
            let item = moth_scene::Item::image(hash, rect, None);
            state
                .request(Request::Apply(Command::Insert(item), Some(handle)))
                .unwrap();
        }
        state.request(Request::EndBatch).unwrap();
        assert_eq!(state.scene.len(), 3);

        match state.request(Request::Undo).unwrap() {
            Some(Completion::AppliedBatch(commands, _)) => assert_eq!(commands.len(), 3),
            _ => panic!("expected an applied batch"),
        }
        assert!(state.scene.is_empty());

        match state.request(Request::Redo).unwrap() {
            Some(Completion::AppliedBatch(commands, _)) => assert_eq!(commands.len(), 3),
            _ => panic!("expected an applied batch"),
        }
        assert_eq!(state.scene.len(), 3);

        state.journal = None;

        state.journal_guard = None;
        state.storage.discard().unwrap();
        std::fs::remove_dir_all(state.storage.base_directory().unwrap()).unwrap();
    }
}
