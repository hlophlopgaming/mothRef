// SPDX-License-Identifier: GPL-3.0-or-later
//! The board library: a worker that keeps the library file (every known board, its
//! pins, tags and app folders) and reads board previews, separate from journal writes.
use moth_format::board;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::Arc,
};

mod catalog;
use catalog::Catalog;
pub use catalog::{FOLDER_COLORS, Folder, MAX_FOLDER_NAME, MAX_FOLDERS};

/// Cards the library can show: every board it remembers.
pub const MAX_BOARDS: usize = catalog::MAX_LIBRARY;
/// Boards whose previews one request may ask for; the screen asks for what it shows.
pub const MAX_PREVIEW_REQUEST: usize = 64;
const MAX_PINS: usize = 20;
/// What the library file says about a board.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    /// Pinned state is stored in the library, never inside a board.
    pub pinned: bool,
    /// Override the global backup preference; `None` follows Settings.
    pub backups: Option<bool>,
    /// Local annotations do not alter the board format.
    pub tags: Vec<String>,
    /// App folders holding the board; the file itself stays where it is.
    pub folders: Vec<uuid::Uuid>,
    /// When the board was last seen opening, in seconds since the Unix epoch.
    pub opened: u64,
}
/// One board's card data.
pub struct Entry {
    /// Desktop identity is kept outside the portable scene and format crates.
    pub path: PathBuf,
    /// What the library file says about it.
    pub meta: Meta,
    /// Items on the board; `None` until the library has read the file once.
    pub count: Option<usize>,
    /// Disk usage can be displayed without stat calls on the UI thread.
    pub bytes: u64,
    /// Last write of the board file, for "edited 2 h ago" and sorting.
    pub modified: Option<std::time::SystemTime>,
    /// The file is not at its path any more (moved, renamed or deleted).
    pub missing: bool,
    /// A saved previous version can be opened even when the original is missing.
    pub backup_available: bool,
    /// A failed board remains visible with an explanation.
    pub error: Option<String>,
}
/// Bounded catalog worker, independent of the active board's storage queue.
pub struct Worker {
    /// Requests from the library screen.
    pub requests: crossbeam_channel::Sender<Request>,
    /// Entries arrive progressively, with backpressure on pixel ownership.
    pub results: crossbeam_channel::Receiver<Result<Update, String>>,
}
/// What the worker reports back to the library screen.
pub enum Update {
    /// Every card at once, from the library file, without reading any board.
    Cards(Vec<Entry>),
    /// A board's card, new or refreshed after its file changed.
    Entry(Entry),
    /// Up to four small images for a card the screen shows.
    Previews(PathBuf, Vec<moth_io::Mip>),
    /// A board's pins, tags or folders changed; its previews did not.
    Meta(PathBuf, Meta),
    /// The app folders, after a refresh or any folder change.
    Folders(Vec<Folder>),
    /// The board left the library (removed from it, moved to the trash or relinked).
    Removed(PathBuf),
    /// Open an immutable backup as an unnamed board.
    Backup {
        original: PathBuf,
        path: Option<PathBuf>,
    },
}
/// Library operations stay off the main thread.
pub enum Request {
    /// Bring in the recent list and send every card again.
    Refresh(Vec<PathBuf>),
    /// Read the previews of these boards (at most [`MAX_PREVIEW_REQUEST`]).
    Previews(Vec<PathBuf>),
    /// Pin or unpin a board.
    Pin(PathBuf, bool),
    /// Override backup creation for this board.
    Backups(PathBuf, Option<bool>),
    /// Locate the latest backup on the worker.
    OpenBackup(PathBuf),
    /// Replace a board's bounded tag list.
    Tags(PathBuf, String),
    /// Show the board file selected in the system file manager.
    Reveal(PathBuf),
    /// Take the board out of the library and the recent list; the file stays.
    Forget(PathBuf),
    /// Move the board file to the system trash, then forget it. Never a permanent delete.
    Trash(PathBuf),
    /// Make an app folder with this name.
    CreateFolder(String),
    /// Rename an app folder.
    RenameFolder(uuid::Uuid, String),
    /// Change an app folder's color (an index into the theme's folder colors).
    RecolorFolder(uuid::Uuid, u8),
    /// Delete an app folder; its boards stay in the library and on disk.
    DeleteFolder(uuid::Uuid),
    /// Put a board into a folder (`true`) or take it out (`false`).
    File {
        /// The board.
        path: PathBuf,
        /// The app folder.
        folder: uuid::Uuid,
        /// Whether the board belongs in it.
        inside: bool,
    },
    /// The board file was moved: point its record at the new path.
    Relink(PathBuf, PathBuf),
}
/// Writes a file of the library's own through a temporary file and an atomic rename.
fn write_file(
    storage: &super::DesktopStorage,
    name: &str,
    bytes: &[u8],
    limit: usize,
) -> std::io::Result<()> {
    use std::io::Write;
    if bytes.len() > limit {
        return Err(std::io::Error::other("Library metadata exceeds its budget"));
    }
    let destination = storage.base_directory()?.join(name);
    let directory = super::parent_directory(&destination);
    std::fs::create_dir_all(directory)?;
    let temporary = directory.join(format!(".catalog-{}.tmp", uuid::Uuid::now_v7()));
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        super::atomic_replace(&temporary, &destination)?;
        super::boards::sync_directory(directory)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
/// The recent list the board worker writes when a board is opened or saved.
fn recent_list(storage: &super::DesktopStorage) -> std::io::Result<Vec<PathBuf>> {
    let mut text = String::new();
    match File::open(storage.base_directory()?.join("recent.txt")) {
        Ok(file) => {
            file.take(64 * 1024).read_to_string(&mut text)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    Ok(text.lines().take(20).map(PathBuf::from).collect())
}
/// Removes a path from the recent list, or replaces it with `to` when the file moved.
fn edit_recent(
    storage: &super::DesktopStorage,
    path: &std::path::Path,
    to: Option<&std::path::Path>,
) -> std::io::Result<()> {
    let recent = recent_list(storage)?;
    if !recent.iter().any(|p| p == path) {
        return Ok(());
    }
    let kept: Vec<_> = recent
        .iter()
        .filter_map(|p| if p == path { to } else { Some(p.as_path()) })
        .filter_map(|p| p.to_str())
        .collect();
    write_file(storage, "recent.txt", kept.join("\n").as_bytes(), 64 * 1024)
}
fn parse_tags(text: &str) -> std::io::Result<Vec<String>> {
    if text.len() > 2048 {
        return Err(std::io::Error::other("Tags exceed the metadata budget"));
    }
    let mut tags = Vec::new();
    for tag in text
        .split([',', '\n'])
        .map(|s| s.trim().trim_start_matches('#'))
        .filter(|s| !s.is_empty())
    {
        let tag = tag.to_lowercase();
        if tag.chars().count() > 32
            || !tag
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(std::io::Error::other(
                "Use up to eight tags, with 32 letters, digits, hyphens or underscores per tag.",
            ));
        }
        if !tags.contains(&tag) {
            tags.push(tag);
        }
        if tags.len() > 8 {
            return Err(std::io::Error::other("A board supports up to eight tags."));
        }
    }
    Ok(tags)
}
fn tag_file(path: &std::path::Path) -> String {
    format!(
        "tags/{}.txt",
        blake3::hash(path.as_os_str().as_encoded_bytes()).to_hex()
    )
}
fn tags(storage: &super::DesktopStorage, path: &std::path::Path) -> std::io::Result<Vec<String>> {
    let file = match File::open(storage.base_directory()?.join(tag_file(path))) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut text = String::new();
    file.take(2049).read_to_string(&mut text)?;
    parse_tags(&text)
}
fn pins(storage: &super::DesktopStorage) -> std::io::Result<Vec<PathBuf>> {
    let path = storage.base_directory()?.join("pins.txt");
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut text = String::new();
    file.take(64 * 1024).read_to_string(&mut text)?;
    Ok(text.lines().take(MAX_PINS).map(PathBuf::from).collect())
}
/// What reading a board file for its card found.
enum Lookup {
    Found(catalog::Summary),
    Missing,
    Failed(String),
}
fn nanos(time: std::time::SystemTime) -> u64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos().min(u64::MAX as u128) as u64)
}
/// The file's size and last write, without opening it.
fn stamp(path: &std::path::Path) -> Result<(u64, u64), Lookup> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok((metadata.len(), metadata.modified().map(nanos).unwrap_or(0))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Lookup::Missing),
        Err(e) => Err(Lookup::Failed(e.to_string())),
    }
}
/// Reads the item count: from the preview at the end of a schema-7 file, or by reading
/// the whole board when there is none (older boards, until their next save).
fn summarize(path: &std::path::Path) -> Lookup {
    let (size, modified) = match stamp(path) {
        Ok(stamp) => stamp,
        Err(read) => return read,
    };
    let result = (|| -> Result<u32, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;
        if let Some(preview) = board::peek_preview(&mut file)? {
            return Ok(preview.items);
        }
        Ok(board::read(&mut file)?.scene.len().min(u32::MAX as usize) as u32)
    })();
    match result {
        Ok(items) => Lookup::Found(catalog::Summary {
            size,
            modified,
            items,
        }),
        Err(error) => Lookup::Failed(error.to_string()),
    }
}
/// Up to four small images for a card: the preview's thumbnails, or for an older board
/// the first originals decoded small.
fn previews(path: &std::path::Path) -> Vec<moth_io::Mip> {
    let decode = |bytes: &[u8]| {
        std::panic::catch_unwind(|| moth_io::decode(bytes, 128))
            .ok()
            .and_then(Result::ok)
            .and_then(|mut image| (!image.mips.is_empty()).then(|| image.mips.remove(0)))
    };
    let result = (|| -> Result<Vec<moth_io::Mip>, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;
        if let Some(preview) = board::peek_preview(&mut file)? {
            return Ok(preview
                .thumbnails
                .iter()
                .filter_map(|thumbnail| decode(&thumbnail.image))
                .collect());
        }
        let board = board::read(&mut file)?;
        let mut images: Vec<_> = board.scene.items().filter(|i| i.blob().is_some()).collect();
        images.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
        let mut hashes = Vec::new();
        for hash in images.into_iter().filter_map(|item| item.blob()) {
            if !hashes.contains(&hash) {
                hashes.push(hash);
            }
            if hashes.len() == board::MAX_THUMBNAILS {
                break;
            }
        }
        let mut mips = Vec::new();
        for hash in hashes {
            let Some(blob) = board.blobs.get(&hash) else {
                continue;
            };
            if blob.length > moth_io::MAX_INPUT_BYTES {
                continue;
            }
            file.seek(SeekFrom::Start(blob.offset))?;
            let mut bytes = Vec::with_capacity(blob.length as usize);
            (&mut file).take(blob.length).read_to_end(&mut bytes)?;
            mips.extend(decode(&bytes));
        }
        Ok(mips)
    })();
    result.unwrap_or_else(|error| {
        tracing::debug!(%error, "board previews unavailable");
        Vec::new()
    })
}
/// A card from what the library file remembers.
fn entry(storage: &super::DesktopStorage, board: &catalog::Board, read: Option<&Lookup>) -> Entry {
    let summary = match read {
        Some(Lookup::Found(summary)) => Some(*summary),
        _ => board.summary,
    };
    Entry {
        path: board.path.clone(),
        meta: Meta {
            pinned: board.pinned,
            backups: board.backups,
            tags: board.tags.clone(),
            folders: board.folders.clone(),
            opened: board.opened,
        },
        count: summary.map(|s| s.items as usize),
        bytes: summary.map_or(0, |s| s.size),
        modified: summary
            .map(|s| std::time::UNIX_EPOCH + std::time::Duration::from_nanos(s.modified)),
        missing: matches!(read, Some(Lookup::Missing)),
        backup_available: matches!(read, Some(Lookup::Missing))
            && super::boards::latest_backup(storage, &board.path)
                .ok()
                .flatten()
                .is_some(),
        error: match read {
            Some(Lookup::Failed(error)) => Some(error.clone()),
            Some(Lookup::Missing) => Some("File not found".into()),
            _ => None,
        },
    }
}
impl Worker {
    /// Starts the library worker without delaying durable edits on another worker.
    pub fn new(
        storage: Arc<super::DesktopStorage>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> std::io::Result<Self> {
        let (requests, rx) = crossbeam_channel::bounded::<Request>(8);
        let (tx, results) = crossbeam_channel::bounded(2);
        std::thread::Builder::new()
            .name("library-index".into())
            .spawn(move || {
                let send = |update: Result<Update, String>| {
                    let alive = tx.send(update).is_ok();
                    wake();
                    alive
                };
                // A library file that cannot be read is never overwritten: the user's
                // folders stay recoverable, and changes this session are not saved.
                let (mut catalog, writable) = match Catalog::load(&storage) {
                    Ok(catalog) => (catalog, true),
                    Err(error) => {
                        tracing::warn!(%error, "library file unreadable; not saving changes");
                        if !send(Err(error.to_string())) {
                            return;
                        }
                        (Catalog::default(), false)
                    }
                };
                let save = |catalog: &Catalog| -> Result<(), String> {
                    if writable {
                        catalog.save(&storage).map_err(|e| e.to_string())
                    } else {
                        Ok(())
                    }
                };
                let meta = |catalog: &Catalog, path: &std::path::Path| {
                    catalog
                        .find(path)
                        .map(|i| {
                            let board = &catalog.boards[i];
                            Meta {
                                pinned: board.pinned,
                                backups: board.backups,
                                tags: board.tags.clone(),
                                folders: board.folders.clone(),
                                opened: board.opened,
                            }
                        })
                        .unwrap_or_default()
                };
                // Boards still to look at after a refresh, newest first. Requests from
                // the screen go before them, so previews are not held up by a long check.
                let mut checks: std::collections::VecDeque<PathBuf> = Default::default();
                let mut dirty = false;
                loop {
                    let request = if checks.is_empty() {
                        if dirty {
                            dirty = false;
                            if let Err(error) = save(&catalog)
                                && !send(Err(error))
                            {
                                return;
                            }
                        }
                        match rx.recv() {
                            Ok(request) => request,
                            Err(_) => return,
                        }
                    } else {
                        match rx.try_recv() {
                            Ok(request) => request,
                            Err(crossbeam_channel::TryRecvError::Disconnected) => return,
                            Err(crossbeam_channel::TryRecvError::Empty) => {
                                let Some(path) = checks.pop_front() else {
                                    continue;
                                };
                                let Some(index) = catalog.find(&path) else {
                                    continue;
                                };
                                let known = catalog.boards[index].summary;
                                let read = match stamp(&path) {
                                    // Unchanged since the library last read it: nothing to do.
                                    Ok((size, modified))
                                        if known.is_some_and(|k| {
                                            k.size == size && k.modified == modified
                                        }) =>
                                    {
                                        continue;
                                    }
                                    Ok(_) => summarize(&path),
                                    Err(read) => read,
                                };
                                if let Lookup::Found(summary) = read {
                                    catalog.boards[index].summary = Some(summary);
                                    dirty = true;
                                }
                                if !send(Ok(Update::Entry(entry(
                                    &storage,
                                    &catalog.boards[index],
                                    Some(&read),
                                )))) {
                                    return;
                                }
                                continue;
                            }
                        }
                    };
                    let result: Result<Vec<Update>, String> = match request {
                        Request::Refresh(recent) => {
                            if catalog.merge_recent(&recent)
                                && let Err(error) = save(&catalog)
                                && !send(Err(error))
                            {
                                return;
                            }
                            if !send(Ok(Update::Folders(catalog.folders.clone()))) {
                                return;
                            }
                            let mut order: Vec<usize> = (0..catalog.boards.len()).collect();
                            order.sort_by_key(|&i| std::cmp::Reverse(catalog.boards[i].opened));
                            let cards = order
                                .iter()
                                .map(|&i| entry(&storage, &catalog.boards[i], None))
                                .collect();
                            checks = order
                                .iter()
                                .map(|&i| catalog.boards[i].path.clone())
                                .collect();
                            tracing::info!(boards = catalog.boards.len(), "library cards ready");
                            if !send(Ok(Update::Cards(cards))) {
                                return;
                            }
                            continue;
                        }
                        Request::Previews(paths) => Ok(paths
                            .into_iter()
                            .take(MAX_PREVIEW_REQUEST)
                            .map(|path| {
                                let mips = previews(&path);
                                Update::Previews(path, mips)
                            })
                            .collect()),
                        Request::Reveal(path) => super::reveal(&path)
                            .map(|()| Vec::new())
                            .map_err(|e| e.to_string()),
                        Request::OpenBackup(original) => {
                            super::boards::latest_backup(&storage, &original)
                                .map(|path| vec![Update::Backup { original, path }])
                                .map_err(|e| e.to_string())
                        }
                        Request::Backups(path, enabled) => catalog
                            .board(&path)
                            .map(|board| board.backups = enabled)
                            .map_err(|e| e.to_string())
                            .and_then(|()| save(&catalog))
                            .map(|()| vec![Update::Meta(path.clone(), meta(&catalog, &path))]),
                        Request::Pin(path, pinned) => catalog
                            .board(&path)
                            .map(|board| board.pinned = pinned)
                            .map_err(|e| e.to_string())
                            .and_then(|()| save(&catalog))
                            .map(|()| vec![Update::Meta(path.clone(), meta(&catalog, &path))]),
                        Request::Tags(path, text) => parse_tags(&text)
                            .and_then(|tags| catalog.board(&path).map(|board| board.tags = tags))
                            .map_err(|e| e.to_string())
                            .and_then(|()| save(&catalog))
                            .map(|()| vec![Update::Meta(path.clone(), meta(&catalog, &path))]),
                        Request::Forget(path) => {
                            catalog.forget(&path);
                            edit_recent(&storage, &path, None)
                                .map_err(|e| e.to_string())
                                .and_then(|()| save(&catalog))
                                .map(|()| vec![Update::Removed(path)])
                        }
                        Request::Trash(path) => match trash::delete(&path) {
                            Ok(()) => {
                                catalog.forget(&path);
                                edit_recent(&storage, &path, None)
                                    .map_err(|e| e.to_string())
                                    .and_then(|()| save(&catalog))
                                    .map(|()| vec![Update::Removed(path)])
                            }
                            Err(error) => Err(error.to_string()),
                        },
                        Request::CreateFolder(name) => catalog
                            .create_folder(&name)
                            .map(drop)
                            .map_err(|e| e.to_string())
                            .and_then(|()| save(&catalog))
                            .map(|()| vec![Update::Folders(catalog.folders.clone())]),
                        Request::RenameFolder(id, name) => catalog
                            .rename_folder(id, &name)
                            .map_err(|e| e.to_string())
                            .and_then(|()| save(&catalog))
                            .map(|()| vec![Update::Folders(catalog.folders.clone())]),
                        Request::RecolorFolder(id, color) => {
                            catalog.recolor_folder(id, color);
                            save(&catalog).map(|()| vec![Update::Folders(catalog.folders.clone())])
                        }
                        Request::DeleteFolder(id) => {
                            catalog.delete_folder(id);
                            save(&catalog).map(|()| vec![Update::Folders(catalog.folders.clone())])
                        }
                        Request::File {
                            path,
                            folder,
                            inside,
                        } => catalog
                            .file(&path, folder, inside)
                            .map_err(|e| e.to_string())
                            .and_then(|()| save(&catalog))
                            .map(|()| vec![Update::Meta(path.clone(), meta(&catalog, &path))]),
                        Request::Relink(from, to) => match summarize(&to) {
                            Lookup::Found(summary) => catalog
                                .relink(&from, &to)
                                .and_then(|()| edit_recent(&storage, &from, Some(&to)))
                                .map_err(|e| e.to_string())
                                .and_then(|()| {
                                    if let Some(index) = catalog.find(&to) {
                                        catalog.boards[index].summary = Some(summary);
                                    }
                                    save(&catalog)
                                })
                                .and_then(|()| {
                                    let index = catalog
                                        .find(&to)
                                        .ok_or_else(|| "The board left the library".to_owned())?;
                                    Ok(vec![
                                        Update::Removed(from),
                                        Update::Entry(entry(
                                            &storage,
                                            &catalog.boards[index],
                                            None,
                                        )),
                                    ])
                                }),
                            Lookup::Missing => Err("File not found".into()),
                            Lookup::Failed(error) => Err(error),
                        },
                    };
                    match result {
                        Ok(updates) => {
                            for update in updates {
                                if !send(Ok(update)) {
                                    return;
                                }
                            }
                        }
                        Err(error) => {
                            if !send(Err(error)) {
                                return;
                            }
                        }
                    }
                }
            })?;
        Ok(Self { requests, results })
    }
}

pub(crate) fn backups_enabled(
    storage: &super::DesktopStorage,
    path: &std::path::Path,
) -> std::io::Result<bool> {
    if let Some(enabled) = catalog::backup_override(storage, path)? {
        return Ok(enabled);
    }
    Ok(super::settings::load(&storage.base_directory()?)?
        .get("backup_copies")
        .is_some_and(|v| v == "true"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn backup_overrides_inherit_and_missing_cards_offer_existing_copies() {
        let root =
            std::env::temp_dir().join(format!("mothref-backup-library-{}", uuid::Uuid::now_v7()));
        let storage = super::super::DesktopStorage::in_directory(root.clone());
        let path = root.join("missing.moth");
        assert!(!backups_enabled(&storage, &path).unwrap());
        super::super::settings::save(&root, "backup_copies", "true").unwrap();
        assert!(backups_enabled(&storage, &path).unwrap());
        let mut catalog = Catalog::load(&storage).unwrap();
        catalog.board(&path).unwrap().backups = Some(false);
        catalog.save(&storage).unwrap();
        assert!(!backups_enabled(&storage, &path).unwrap());
        catalog.board(&path).unwrap().backups = Some(true);
        catalog.save(&storage).unwrap();
        super::super::settings::save(&root, "backup_copies", "false").unwrap();
        assert!(backups_enabled(&storage, &path).unwrap());
        catalog.board(&path).unwrap().backups = None;
        catalog.save(&storage).unwrap();
        assert!(!backups_enabled(&storage, &path).unwrap());
        let board = catalog.board(&path).unwrap();
        assert!(!entry(&storage, board, Some(&Lookup::Missing)).backup_available);
        let directory = root.join("backups").join(
            blake3::hash(path.as_os_str().as_encoded_bytes())
                .to_hex()
                .as_str(),
        );
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join(format!("{}.moth", uuid::Uuid::now_v7())),
            include_bytes!("../../../corpus/moth/v7.moth"),
        )
        .unwrap();
        let card = entry(&storage, board, Some(&Lookup::Missing));
        assert!(
            card.missing && card.backup_available,
            "old copies remain accessible when creation is disabled"
        );
        assert!(!entry(&storage, board, None).backup_available);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn the_first_library_file_keeps_recent_boards_pins_and_tags() {
        let root = std::env::temp_dir().join(format!("mothref-catalog-{}", uuid::Uuid::now_v7()));
        let storage = super::super::DesktopStorage::in_directory(root.clone());
        let path = PathBuf::from("example.moth");
        std::fs::create_dir_all(root.join("tags")).unwrap();
        std::fs::write(root.join("recent.txt"), "example.moth\nother.moth").unwrap();
        std::fs::write(root.join("pins.txt"), "pinned.moth").unwrap();
        std::fs::write(root.join(tag_file(&path)), "armor\nwip").unwrap();
        let catalog = Catalog::load(&storage).unwrap();
        let paths: Vec<_> = catalog.boards.iter().map(|b| b.path.clone()).collect();
        assert_eq!(
            paths,
            ["example.moth", "other.moth", "pinned.moth"].map(PathBuf::from)
        );
        assert_eq!(catalog.boards[0].tags, ["armor", "wip"]);
        assert!(catalog.boards[2].pinned);
        catalog.save(&storage).unwrap();
        assert_eq!(Catalog::load(&storage).unwrap(), catalog);
        // Forgetting a board edits the recent list too.
        edit_recent(&storage, &path, None).unwrap();
        assert_eq!(
            recent_list(&storage).unwrap(),
            [PathBuf::from("other.moth")]
        );
        edit_recent(
            &storage,
            Path::new("other.moth"),
            Some(Path::new("moved.moth")),
        )
        .unwrap();
        assert_eq!(
            recent_list(&storage).unwrap(),
            [PathBuf::from("moved.moth")]
        );
        assert!(parse_tags(&"a".repeat(2049)).is_err());
        assert!(parse_tags("one,two,three,four,five,six,seven,eight,nine").is_err());
        assert_eq!(parse_tags("#Armor, WIP,armor").unwrap(), ["armor", "wip"]);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cards_come_from_the_preview_or_the_whole_board_and_missing_files_say_so() {
        let path = std::env::temp_dir().join(format!("missing-{}.moth", uuid::Uuid::now_v7()));
        assert!(matches!(summarize(&path), Lookup::Missing));
        assert!(previews(&path).is_empty());
        // The schema-7 fixture answers from its preview; the schema-6 one is read whole.
        let root = std::env::temp_dir().join(format!("mothref-cards-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&root).unwrap();
        for (name, bytes) in [
            (
                "v7.moth",
                include_bytes!("../../../corpus/moth/v7.moth").as_slice(),
            ),
            (
                "v6.moth",
                include_bytes!("../../../corpus/moth/v6.moth").as_slice(),
            ),
        ] {
            let path = root.join(name);
            std::fs::write(&path, bytes).unwrap();
            let Lookup::Found(summary) = summarize(&path) else {
                panic!("{name} has a card");
            };
            assert_eq!(summary.items, 6, "{name}");
            assert_eq!(summary.size, bytes.len() as u64);
            assert_eq!(previews(&path).len(), 1, "{name}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
