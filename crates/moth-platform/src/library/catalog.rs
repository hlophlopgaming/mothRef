// SPDX-License-Identifier: GPL-3.0-or-later
//! The library file: every board the user has opened or added, with its pins, tags and
//! the app's own folders. Folders only group paths; board files never move on disk.
use super::*;
use serde_json::{Value, json};
use std::{collections::HashSet, path::Path};
use uuid::Uuid;

/// Name of the library file in the application data directory.
const FILE: &str = "library.json";
/// The library file stays small enough to read and rewrite on the worker in a blink.
const MAX_BYTES: usize = 64 * 1024 * 1024;
/// Boards remembered by the library; the oldest-opened go first when it is full.
pub(super) const MAX_LIBRARY: usize = 100_000;
/// Folders the user can create.
pub const MAX_FOLDERS: usize = 1000;
/// Longest folder name, in characters.
pub const MAX_FOLDER_NAME: usize = 64;
/// How many folder colors the theme offers; a folder stores an index into them.
pub const FOLDER_COLORS: u8 = 6;

/// A folder made in the app. It lists boards by path; one board can be in several.
#[derive(Clone, Debug, PartialEq)]
pub struct Folder {
    /// Stable identity, so renaming keeps the boards in it.
    pub id: Uuid,
    /// Name shown in the sidebar.
    pub name: String,
    /// Index into the theme's folder colors.
    pub color: u8,
}

/// What the library remembers about one board file.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Board {
    pub path: PathBuf,
    pub pinned: bool,
    pub backups: Option<bool>,
    pub tags: Vec<String>,
    pub folders: Vec<Uuid>,
    /// Seconds since the Unix epoch when the board was last seen opening.
    pub opened: u64,
    /// What the file held when the library last looked, so a card shows without
    /// reading the board again while the file is unchanged.
    pub summary: Option<Summary>,
}
/// A board file as the library last read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    /// File size in bytes.
    pub size: u64,
    /// Last write of the file, in nanoseconds since the Unix epoch.
    pub modified: u64,
    /// Items on the board.
    pub items: u32,
}

/// The whole library file, kept in memory by the library worker.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Catalog {
    pub folders: Vec<Folder>,
    pub boards: Vec<Board>,
}

pub(super) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Catalog {
    /// Reads the library file; on first use builds it from the older recent list, pins
    /// and tag files, so nothing the user set up is lost.
    pub fn load(storage: &super::super::DesktopStorage) -> std::io::Result<Self> {
        let path = storage.base_directory()?.join(FILE);
        match File::open(&path) {
            Ok(file) => {
                let mut text = String::new();
                file.take(MAX_BYTES as u64 + 1).read_to_string(&mut text)?;
                if text.len() > MAX_BYTES {
                    return Err(std::io::Error::other("The library file exceeds its budget"));
                }
                Self::parse(&text)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let catalog = Self::migrate(storage)?;
                // Written at once, so the older lists are read only this one time.
                if let Err(error) = catalog.save(storage) {
                    tracing::warn!(%error, "library file not written");
                }
                Ok(catalog)
            }
            Err(e) => Err(e),
        }
    }
    fn migrate(storage: &super::super::DesktopStorage) -> std::io::Result<Self> {
        let mut catalog = Self::default();
        let start = now();
        let recent = recent_list(storage)?;
        let pinned = pins(storage)?;
        for (rank, path) in recent.iter().chain(&pinned).enumerate() {
            if catalog.find(path).is_some() {
                continue;
            }
            catalog.boards.push(Board {
                path: path.clone(),
                pinned: pinned.contains(path),
                backups: None,
                tags: tags(storage, path).unwrap_or_default(),
                folders: Vec::new(),
                opened: start.saturating_sub(rank as u64),
                summary: None,
            });
        }
        Ok(catalog)
    }
    pub fn parse(text: &str) -> std::io::Result<Self> {
        let value: Value = serde_json::from_str(text).map_err(std::io::Error::other)?;
        let mut catalog = Self::default();
        for folder in value["folders"].as_array().into_iter().flatten() {
            let (Some(id), Some(name)) = (
                folder["id"]
                    .as_str()
                    .and_then(|id| Uuid::parse_str(id).ok()),
                folder["name"].as_str().and_then(folder_name),
            ) else {
                continue;
            };
            if catalog.folders.len() < MAX_FOLDERS && catalog.folder(id).is_none() {
                catalog.folders.push(Folder {
                    id,
                    name,
                    color: folder["color"]
                        .as_u64()
                        .map_or(0, |c| (c % FOLDER_COLORS as u64) as u8),
                });
            }
        }
        for board in value["boards"].as_array().into_iter().flatten() {
            let Some(path) = board["path"]
                .as_str()
                .filter(|p| valid_path(p))
                .map(PathBuf::from)
            else {
                continue;
            };
            if catalog.boards.len() >= MAX_LIBRARY || catalog.find(&path).is_some() {
                continue;
            }
            let tags = board["tags"]
                .as_array()
                .map(|tags| {
                    tags.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .and_then(|text| parse_tags(&text).ok())
                .unwrap_or_default();
            let mut folders: Vec<Uuid> = board["folders"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|id| id.as_str().and_then(|id| Uuid::parse_str(id).ok()))
                .filter(|id| catalog.folder(*id).is_some())
                .collect();
            folders.dedup();
            catalog.boards.push(Board {
                path,
                pinned: board["pinned"].as_bool().unwrap_or(false),
                backups: board["backups"].as_bool(),
                tags,
                folders,
                opened: board["opened"].as_u64().unwrap_or(0),
                summary: match (
                    board["size"].as_u64(),
                    board["modified"].as_u64(),
                    board["items"].as_u64().and_then(|n| u32::try_from(n).ok()),
                ) {
                    (Some(size), Some(modified), Some(items)) => Some(Summary {
                        size,
                        modified,
                        items,
                    }),
                    _ => None,
                },
            });
        }
        Ok(catalog)
    }
    pub fn to_json(&self) -> String {
        json!({
            "version": 1,
            "folders": self.folders.iter().map(|f| json!({
                "id": f.id.to_string(),
                "name": f.name,
                "color": f.color,
            })).collect::<Vec<_>>(),
            "boards": self.boards.iter().filter_map(|b| {
                let mut record = json!({
                    "path": b.path.to_str()?,
                    "pinned": b.pinned,
                    "backups": b.backups,
                    "tags": b.tags,
                    "folders": b.folders.iter().map(Uuid::to_string).collect::<Vec<_>>(),
                    "opened": b.opened,
                });
                if let Some(summary) = b.summary {
                    record["size"] = summary.size.into();
                    record["modified"] = summary.modified.into();
                    record["items"] = summary.items.into();
                }
                Some(record)
            }).collect::<Vec<_>>(),
        })
        .to_string()
    }
    pub fn save(&self, storage: &super::super::DesktopStorage) -> std::io::Result<()> {
        write_file(storage, FILE, self.to_json().as_bytes(), MAX_BYTES)
    }
    pub fn find(&self, path: &Path) -> Option<usize> {
        self.boards.iter().position(|b| b.path == path)
    }
    pub fn folder(&self, id: Uuid) -> Option<&Folder> {
        self.folders.iter().find(|f| f.id == id)
    }
    /// The board's record, added (as just opened) when the library did not know it.
    pub fn board(&mut self, path: &Path) -> std::io::Result<&mut Board> {
        if !path.to_str().is_some_and(valid_path) {
            return Err(std::io::Error::other("Unsupported board path"));
        }
        let index = match self.find(path) {
            Some(index) => index,
            None => {
                self.make_room();
                self.boards.push(Board {
                    path: path.to_owned(),
                    pinned: false,
                    backups: None,
                    tags: Vec::new(),
                    folders: Vec::new(),
                    opened: now(),
                    summary: None,
                });
                self.boards.len() - 1
            }
        };
        Ok(&mut self.boards[index])
    }
    fn make_room(&mut self) {
        while self.boards.len() >= MAX_LIBRARY {
            let Some(oldest) = self
                .boards
                .iter()
                .enumerate()
                .filter(|(_, b)| !b.pinned && b.folders.is_empty())
                .min_by_key(|(_, b)| b.opened)
                .map(|(i, _)| i)
            else {
                return;
            };
            self.boards.remove(oldest);
        }
    }
    /// Brings in the boards of the recent list; the newest one counts as just opened.
    /// Returns whether anything changed.
    pub fn merge_recent(&mut self, recent: &[PathBuf]) -> bool {
        let start = now();
        let newest = self.boards.iter().map(|b| b.opened).max().unwrap_or(0);
        let mut changed = false;
        for (rank, path) in recent.iter().enumerate() {
            if !path.to_str().is_some_and(valid_path) {
                continue;
            }
            match self.find(path) {
                Some(index) => {
                    // Only the top of the recent list moves: it is the board opened last.
                    if rank == 0 && self.boards[index].opened < newest {
                        self.boards[index].opened = start.max(newest + 1);
                        changed = true;
                    }
                }
                None => {
                    self.make_room();
                    self.boards.push(Board {
                        path: path.clone(),
                        pinned: false,
                        backups: None,
                        tags: Vec::new(),
                        folders: Vec::new(),
                        opened: start.saturating_sub(rank as u64),
                        summary: None,
                    });
                    changed = true;
                }
            }
        }
        changed
    }
    pub fn create_folder(&mut self, name: &str) -> std::io::Result<Uuid> {
        let name = folder_name(name).ok_or_else(|| std::io::Error::other("Name the folder"))?;
        if self.folders.len() >= MAX_FOLDERS {
            return Err(std::io::Error::other(
                "The library holds up to 1,000 folders",
            ));
        }
        let id = Uuid::now_v7();
        // Colors first, the neutral gray (index 0) last.
        let used: HashSet<u8> = self.folders.iter().map(|f| f.color).collect();
        let palette = (1..FOLDER_COLORS).chain([0]);
        let color = palette
            .clone()
            .find(|c| !used.contains(c))
            .or_else(|| {
                palette
                    .clone()
                    .nth(self.folders.len() % FOLDER_COLORS as usize)
            })
            .unwrap_or(0);
        self.folders.push(Folder { id, name, color });
        Ok(id)
    }
    pub fn rename_folder(&mut self, id: Uuid, name: &str) -> std::io::Result<()> {
        let name = folder_name(name).ok_or_else(|| std::io::Error::other("Name the folder"))?;
        self.folders
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or_else(|| std::io::Error::other("The folder no longer exists"))?
            .name = name;
        Ok(())
    }
    pub fn recolor_folder(&mut self, id: Uuid, color: u8) {
        if let Some(folder) = self.folders.iter_mut().find(|f| f.id == id) {
            folder.color = color % FOLDER_COLORS;
        }
    }
    /// Deletes the folder only; its boards stay in the library and on disk.
    pub fn delete_folder(&mut self, id: Uuid) {
        self.folders.retain(|f| f.id != id);
        for board in &mut self.boards {
            board.folders.retain(|f| *f != id);
        }
    }
    pub fn file(&mut self, path: &Path, folder: Uuid, inside: bool) -> std::io::Result<()> {
        if self.folder(folder).is_none() {
            return Err(std::io::Error::other("The folder no longer exists"));
        }
        let board = self.board(path)?;
        board.folders.retain(|f| *f != folder);
        if inside {
            board.folders.push(folder);
        }
        Ok(())
    }
    pub fn forget(&mut self, path: &Path) {
        self.boards.retain(|b| b.path != path);
    }
    /// Points the record at the file's new place, keeping pins, tags and folders.
    pub fn relink(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        if !to.to_str().is_some_and(valid_path) {
            return Err(std::io::Error::other("Unsupported board path"));
        }
        let old = self
            .find(from)
            .ok_or_else(|| std::io::Error::other("The board is no longer in the library"))?;
        if let Some(existing) = self.find(to).filter(|&i| i != old) {
            // The new place was known already: merge the two records into it.
            let previous = self.boards.remove(old);
            let index = if existing > old {
                existing - 1
            } else {
                existing
            };
            let target = &mut self.boards[index];
            target.pinned |= previous.pinned;
            target.backups = target.backups.or(previous.backups);
            for tag in previous.tags {
                if !target.tags.contains(&tag) && target.tags.len() < 8 {
                    target.tags.push(tag);
                }
            }
            for folder in previous.folders {
                if !target.folders.contains(&folder) {
                    target.folders.push(folder);
                }
            }
            target.opened = target.opened.max(previous.opened);
        } else {
            self.boards[old].path = to.to_owned();
            self.boards[old].summary = None;
        }
        Ok(())
    }
}

/// Paths are stored as text; one that cannot be (a line break, NUL) is not remembered.
fn valid_path(path: &str) -> bool {
    !path.is_empty() && !path.contains(['\n', '\r', '\0'])
}

/// A trimmed folder name without control characters, or `None` when nothing is left.
pub(super) fn folder_name(name: &str) -> Option<String> {
    let name: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_FOLDER_NAME)
        .collect();
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Read only the override on the storage worker; do not rebuild the catalog on every save.
pub(super) fn backup_override(
    storage: &super::super::DesktopStorage,
    path: &Path,
) -> std::io::Result<Option<bool>> {
    let file = match File::open(storage.base_directory()?.join(FILE)) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(std::io::Error::other("Library metadata exceeds its budget"));
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    Ok(value["boards"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|b| b["path"].as_str().is_some_and(|p| Path::new(p) == path))
        .and_then(|b| b["backups"].as_bool()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path(name: &str) -> PathBuf {
        PathBuf::from(format!("/boards/{name}.moth"))
    }
    #[test]
    fn backup_overrides_roundtrip_and_survive_relinking() {
        let mut catalog = Catalog::parse(r#"{"boards":[{"path":"/boards/old.moth"}]}"#).unwrap();
        assert_eq!(catalog.boards[0].backups, None);
        for value in [Some(true), Some(false), None] {
            catalog.boards[0].backups = value;
            assert_eq!(
                Catalog::parse(&catalog.to_json()).unwrap().boards[0].backups,
                value
            );
        }
        catalog.boards[0].backups = Some(true);
        catalog.board(&path("new")).unwrap();
        catalog.relink(&path("old"), &path("new")).unwrap();
        assert_eq!(catalog.boards[0].backups, Some(true));
    }
    #[test]
    fn folders_group_paths_and_survive_a_round_trip() {
        let mut catalog = Catalog::default();
        assert!(catalog.merge_recent(&[path("a"), path("b")]));
        let folder = catalog.create_folder("  Characters\n").unwrap();
        assert_eq!(catalog.folders[0].name, "Characters");
        catalog.file(&path("a"), folder, true).unwrap();
        catalog.file(&path("c"), folder, true).unwrap();
        catalog.board(&path("b")).unwrap().pinned = true;
        let read = Catalog::parse(&catalog.to_json()).unwrap();
        assert_eq!(read, catalog);
        assert_eq!(
            read.boards
                .iter()
                .filter(|b| b.folders.contains(&folder))
                .count(),
            2
        );
        // Deleting the folder keeps every board.
        catalog.delete_folder(folder);
        assert_eq!(catalog.boards.len(), 3);
        assert!(catalog.boards.iter().all(|b| b.folders.is_empty()));
        assert!(catalog.create_folder(" \t").is_err());
    }
    #[test]
    fn the_top_of_the_recent_list_becomes_the_newest_board() {
        let mut catalog = Catalog::default();
        catalog.merge_recent(&[path("a"), path("b")]);
        assert!(!catalog.merge_recent(&[path("a"), path("b")]));
        assert!(catalog.merge_recent(&[path("b"), path("a")]));
        let newest = catalog.boards.iter().max_by_key(|b| b.opened).unwrap();
        assert_eq!(newest.path, path("b"));
    }
    #[test]
    fn relinking_keeps_folders_and_merges_with_a_known_target() {
        let mut catalog = Catalog::default();
        let folder = catalog.create_folder("Refs").unwrap();
        catalog.file(&path("old"), folder, true).unwrap();
        catalog.relink(&path("old"), &path("new")).unwrap();
        assert_eq!(catalog.boards[0].path, path("new"));
        assert_eq!(catalog.boards[0].folders, [folder]);
        catalog.board(&path("other")).unwrap().pinned = true;
        catalog.relink(&path("other"), &path("new")).unwrap();
        assert_eq!(catalog.boards.len(), 1);
        assert!(catalog.boards[0].pinned);
    }
    #[test]
    fn a_damaged_file_drops_only_bad_entries() {
        let text = r#"{"folders":[{"id":"not-a-uuid","name":"x"},{"id":"0190b3c4-0000-7000-8000-000000000001","name":"Ok","color":9}],
            "boards":[{"path":"/a.moth","folders":["0190b3c4-0000-7000-8000-000000000001","0190b3c4-0000-7000-8000-000000000002"],"tags":["Wip","bad tag!"]},{"path":""},{"nope":1}]}"#;
        let catalog = Catalog::parse(text).unwrap();
        assert_eq!(catalog.folders.len(), 1);
        assert_eq!(catalog.folders[0].color, 3);
        assert_eq!(catalog.boards.len(), 1);
        assert_eq!(catalog.boards[0].folders.len(), 1);
        // A tag list that fails validation is dropped as a whole, not half-applied.
        assert!(catalog.boards[0].tags.is_empty());
    }
}
