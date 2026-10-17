// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only library previews run separately from journal writes.
use moth_format::board;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::Arc,
};

/// The recent-file catalog bounds both metadata and preview ownership.
pub const MAX_BOARDS: usize = 40;
const MAX_PINS: usize = 20;
/// Four 128-pixel previews per board bound the catalog to 10 MiB of pixels.
pub struct Entry {
    /// Desktop identity is kept outside the portable scene and format crates.
    pub path: PathBuf,
    /// Pinned state is stored in the app catalog, never inside a board.
    pub pinned: bool,
    /// Local catalog annotations do not alter the board format.
    pub tags: Vec<String>,
    /// Count is read without retaining the scene.
    pub count: usize,
    /// Disk usage can be displayed without stat calls on the UI thread.
    pub bytes: u64,
    /// A failed board remains visible with an explanation.
    pub error: Option<String>,
    /// Rebuildable previews never replace the originals.
    pub previews: Vec<moth_io::Mip>,
}
/// Bounded catalog worker, independent of the active board's storage queue.
pub struct Worker {
    /// Refresh requests contain at most twenty native paths.
    pub requests: crossbeam_channel::Sender<Request>,
    /// Entries arrive progressively, with backpressure on pixel ownership.
    pub results: crossbeam_channel::Receiver<Result<Entry, String>>,
}
/// Catalog operations stay off the main thread.
pub enum Request {
    /// Rebuild previews for the bounded recent-file list.
    Refresh(Vec<PathBuf>),
    /// Persist a catalog pin without modifying the board.
    Pin(PathBuf, bool),
    /// Atomically replace a board's bounded tag list.
    Tags(PathBuf, String),
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
fn write_catalog(storage: &super::DesktopStorage, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if bytes.len() > 64 * 1024 {
        return Err(std::io::Error::other("Catalog metadata exceeds its budget"));
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
fn pin(storage: &super::DesktopStorage, path: PathBuf, enabled: bool) -> std::io::Result<()> {
    let mut paths = pins(storage)?;
    paths.retain(|p| p != &path);
    if enabled {
        if path.to_str().is_none_or(|s| s.contains(['\n', '\r'])) {
            return Err(std::io::Error::other("Unsupported catalog path"));
        }
        paths.insert(0, path);
        paths.truncate(MAX_PINS);
    }
    write_catalog(
        storage,
        "pins.txt",
        paths
            .iter()
            .filter_map(|p| p.to_str())
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    )
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
fn inspect(path: PathBuf) -> Entry {
    let mut entry = Entry {
        path,
        pinned: false,
        tags: Vec::new(),
        count: 0,
        bytes: 0,
        error: None,
        previews: Vec::new(),
    };
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut file = File::open(&entry.path)?;
        entry.bytes = file.metadata()?.len();
        let board = board::read(&mut file)?;
        entry.count = board.scene.len();
        let mut hashes = Vec::new();
        for item in board.scene.items() {
            if !hashes.contains(&item.blob) {
                hashes.push(item.blob);
            }
            if hashes.len() == 4 {
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
    /// Starts read-only indexing without delaying durable edits on another worker.
    pub fn new(
        storage: Arc<super::DesktopStorage>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> std::io::Result<Self> {
        let (requests, rx) = crossbeam_channel::bounded::<Request>(2);
        let (tx, results) = crossbeam_channel::bounded(2);
        std::thread::Builder::new()
            .name("library-index".into())
            .spawn(move || {
                while let Ok(request) = rx.recv() {
                    let paths = match request {
                        Request::Refresh(paths) => paths,
                        Request::Tags(path, text) => {
                            let result = parse_tags(&text).and_then(|tags| {
                                write_catalog(
                                    &storage,
                                    &tag_file(&path),
                                    tags.join("\n").as_bytes(),
                                )
                            });
                            if let Err(error) = result {
                                if tx.send(Err(error.to_string())).is_err() {
                                    return;
                                }
                                wake();
                                continue;
                            }
                            vec![path]
                        }
                        Request::Pin(path, enabled) => {
                            if let Err(error) = pin(&storage, path.clone(), enabled) {
                                if tx.send(Err(error.to_string())).is_err() {
                                    return;
                                }
                                wake();
                                continue;
                            }
                            vec![path]
                        }
                    };
                    let pinned = match pins(&storage) {
                        Ok(pins) => pins,
                        Err(error) => {
                            if tx.send(Err(error.to_string())).is_err() {
                                return;
                            }
                            wake();
                            Vec::new()
                        }
                    };
                    let mut paths = paths.into_iter().take(20).collect::<Vec<_>>();
                    for path in &pinned {
                        if !paths.contains(path) {
                            paths.push(path.clone());
                        }
                    }
                    for path in paths.into_iter().take(MAX_BOARDS) {
                        let mut entry = inspect(path);
                        entry.pinned = pinned.contains(&entry.path);
                        match tags(&storage, &entry.path) {
                            Ok(tags) => entry.tags = tags,
                            Err(error) => {
                                if tx.send(Err(error.to_string())).is_err() {
                                    return;
                                }
                                wake();
                            }
                        }
                        if tx.send(Ok(entry)).is_err() {
                            return;
                        }
                        wake();
                    }
                }
            })?;
        Ok(Self { requests, results })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_annotations_survive_reopening_and_reject_oversized_input() {
        let root = std::env::temp_dir().join(format!("mothref-catalog-{}", uuid::Uuid::now_v7()));
        let storage = super::super::DesktopStorage::in_directory(root.clone());
        let path = PathBuf::from("example.moth");
        pin(&storage, path.clone(), true).unwrap();
        assert_eq!(pins(&storage).unwrap(), vec![path.clone()]);
        let normalized = parse_tags("#Armor, WIP,armor").unwrap();
        assert_eq!(normalized, vec!["armor", "wip"]);
        write_catalog(&storage, &tag_file(&path), normalized.join("\n").as_bytes()).unwrap();
        assert_eq!(tags(&storage, &path).unwrap(), normalized);
        assert!(parse_tags(&"a".repeat(2049)).is_err());
        assert!(parse_tags("one,two,three,four,five,six,seven,eight,nine").is_err());
        assert_eq!(tags(&storage, &path).unwrap(), normalized);
        pin(&storage, path.clone(), false).unwrap();
        assert!(pins(&storage).unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_board_stays_in_catalog_as_an_error() {
        let path = std::env::temp_dir().join(format!("missing-{}.moth", uuid::Uuid::now_v7()));
        let entry = inspect(path.clone());
        assert_eq!(entry.path, path);
        assert!(entry.error.is_some());
        assert!(entry.previews.is_empty());
    }
}
