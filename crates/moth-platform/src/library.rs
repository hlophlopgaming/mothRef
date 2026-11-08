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

/// Cards sent per refresh, most recently opened first.
pub const MAX_BOARDS: usize = 300;
/// The newest boards get four previews; the rest one, which bounds preview pixels to
/// about 30 MiB however large the library grows.
const FULL_PREVIEWS: usize = 48;
const MAX_PINS: usize = 20;
/// What the library file says about a board.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    /// Pinned state is stored in the library, never inside a board.
    pub pinned: bool,
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
    /// Count is read without retaining the scene.
    pub count: usize,
    /// Disk usage can be displayed without stat calls on the UI thread.
    pub bytes: u64,
    /// Last write of the board file, for "edited 2 h ago" and sorting.
    pub modified: Option<std::time::SystemTime>,
    /// The file is not at its path any more (moved, renamed or deleted).
    pub missing: bool,
    /// A failed board remains visible with an explanation.
    pub error: Option<String>,
    /// Rebuildable previews never replace the originals.
    pub previews: Vec<moth_io::Mip>,
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
    /// A board's card, new or refreshed.
    Entry(Entry),
    /// A board's pins, tags or folders changed; its previews did not.
    Meta(PathBuf, Meta),
    /// The app folders, after a refresh or any folder change.
    Folders(Vec<Folder>),
    /// The board left the library (removed from it, moved to the trash or relinked).
    Removed(PathBuf),
}
/// Library operations stay off the main thread.
pub enum Request {
    /// Bring in the recent list and send every card again.
    Refresh(Vec<PathBuf>),
    /// Pin or unpin a board.
    Pin(PathBuf, bool),
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
fn inspect(path: PathBuf, previews: usize) -> Entry {
    let mut entry = Entry {
        path,
        meta: Meta::default(),
        count: 0,
        bytes: 0,
        modified: None,
        missing: false,
        error: None,
        previews: Vec::new(),
    };
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut file = match File::open(&entry.path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                entry.missing = true;
                return Err(e.into());
            }
            file => file?,
        };
        let metadata = file.metadata()?;
        entry.bytes = metadata.len();
        entry.modified = metadata.modified().ok();
        let board = board::read(&mut file)?;
        entry.count = board.scene.len();
        let mut hashes = Vec::new();
        for item in board.scene.items() {
            let Some(blob) = item.blob() else {
                continue;
            };
            if !hashes.contains(&blob) {
                hashes.push(blob);
            }
            if hashes.len() == previews {
                break;
            }
        }
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
            if let Ok(mut image) = moth_io::decode(&bytes, 128) {
                entry.previews.push(image.mips.remove(0));
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        entry.error = Some(error.to_string());
    }
    entry
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
                                tags: board.tags.clone(),
                                folders: board.folders.clone(),
                                opened: board.opened,
                            }
                        })
                        .unwrap_or_default()
                };
                while let Ok(request) = rx.recv() {
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
                            for (rank, index) in order.into_iter().take(MAX_BOARDS).enumerate() {
                                let path = catalog.boards[index].path.clone();
                                let previews = if rank < FULL_PREVIEWS { 4 } else { 1 };
                                let mut entry = inspect(path, previews);
                                entry.meta = meta(&catalog, &entry.path);
                                if !send(Ok(Update::Entry(entry))) {
                                    return;
                                }
                            }
                            continue;
                        }
                        Request::Reveal(path) => super::reveal(&path)
                            .map(|()| Vec::new())
                            .map_err(|e| e.to_string()),
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
                        Request::Relink(from, to) => {
                            let mut entry = inspect(to.clone(), 4);
                            if let Some(error) = entry.error.take() {
                                Err(error)
                            } else {
                                catalog
                                    .relink(&from, &to)
                                    .and_then(|()| edit_recent(&storage, &from, Some(&to)))
                                    .map_err(|e| e.to_string())
                                    .and_then(|()| save(&catalog))
                                    .map(|()| {
                                        entry.meta = meta(&catalog, &to);
                                        vec![Update::Removed(from), Update::Entry(entry)]
                                    })
                            }
                        }
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
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
    fn missing_board_stays_in_catalog_as_missing() {
        let path = std::env::temp_dir().join(format!("missing-{}.moth", uuid::Uuid::now_v7()));
        let entry = inspect(path.clone(), 4);
        assert_eq!(entry.path, path);
        assert!(entry.missing);
        assert!(entry.error.is_some());
        assert!(entry.previews.is_empty());
    }
}
