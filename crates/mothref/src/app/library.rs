// SPDX-License-Identifier: GPL-3.0-or-later
//! The library screen: its cards, folders and bounded preview textures, kept in step
//! with the library worker. The app forwards worker updates here and acts only on
//! the [`Effect`]s that reach beyond the screen (tabs, the recent list, notices).
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
};

use moth_platform::library::{Request, Update};
use moth_ui::library::{Card, Folder, View};

/// Cards that keep their preview textures; scrolling further gives the oldest back
/// (about 16 MiB of 128-pixel previews).
pub(super) const MAX_PREVIEW_CARDS: usize = 64;

/// What the workspace knows about boards, for building cards.
pub(super) struct Boards<'a> {
    /// The recent list, most recent first.
    pub recent: &'a [PathBuf],
    /// Board files with changes waiting in a recovery journal.
    pub unsaved: Vec<&'a Path>,
}

/// A library update the screen cannot settle by itself.
#[derive(Debug, PartialEq)]
pub(super) enum Effect {
    /// Open this backup of `original` as an unnamed board.
    OpenBackup { original: PathBuf, path: PathBuf },
    /// The backup asked for is gone.
    BackupUnavailable,
    /// A board found with "Find…" moved: the recent list follows it.
    Moved { old: PathBuf, new: PathBuf },
    /// A board left the library: the recent list forgets it.
    Forgotten(PathBuf),
}

pub(super) struct LibraryScreen {
    pub worker: moth_platform::library::Worker,
    pub catalog: moth_ui::library::Library,
    cards: Vec<Card>,
    /// The app folders, as the library worker last reported them.
    folders: Vec<Folder>,
    /// "Find…" for a missing board: its old path and the system dialog's answer.
    relink: Option<(PathBuf, crossbeam_channel::Receiver<Option<PathBuf>>)>,
    /// A relink sent to the worker: its answer moves the board in the recent list too.
    relinking: Option<(PathBuf, PathBuf)>,
    /// Cards holding preview textures, least recently loaded first.
    preview_order: VecDeque<PathBuf>,
}

impl LibraryScreen {
    pub(super) fn new(worker: moth_platform::library::Worker) -> Self {
        Self {
            worker,
            cards: Vec::new(),
            catalog: Default::default(),
            folders: Vec::new(),
            relink: None,
            relinking: None,
            preview_order: Default::default(),
        }
    }
    /// What the screen draws: the catalog widget state, the cards and the folders.
    pub(super) fn parts(&mut self) -> (&mut moth_ui::library::Library, &[Card], &[Folder]) {
        (&mut self.catalog, &self.cards, &self.folders)
    }
    /// Leaving the screen gives every preview texture back; the next visit asks the
    /// worker again.
    pub(super) fn hide(&mut self) {
        self.cards.clear();
        self.preview_order.clear();
        self.catalog.dismiss_editor();
    }
    /// Sends a request to the library worker; `false` when its queue is full.
    pub(super) fn send(&self, request: Request) -> bool {
        self.worker.requests.try_send(request).is_ok()
    }
    /// Asks for the previews of the cards on screen, marking them as loading.
    pub(super) fn request_previews(&mut self, paths: Vec<PathBuf>) {
        let paths: Vec<_> = paths
            .into_iter()
            .take(moth_platform::library::MAX_PREVIEW_REQUEST)
            .collect();
        // A full queue just means the cards ask again next frame.
        if self.send(Request::Previews(paths.clone())) {
            for card in &mut self.cards {
                if paths.contains(&card.path) {
                    card.loading = true;
                }
            }
        }
    }
    /// Whether "Find…" is already waiting for the system dialog.
    pub(super) fn finding(&self) -> bool {
        self.relink.is_some()
    }
    /// Remembers the open "Find…" dialog for `old` and where its answer arrives.
    pub(super) fn find(
        &mut self,
        old: PathBuf,
        answer: crossbeam_channel::Receiver<Option<PathBuf>>,
    ) {
        self.relink = Some((old, answer));
    }
    /// Sends the answer of a finished "Find…" to the worker; `false` when the worker
    /// was too busy to take it.
    pub(super) fn poll_find(&mut self) -> bool {
        let Some((old, answer)) = &self.relink else {
            return true;
        };
        let Ok(picked) = answer.try_recv() else {
            return true;
        };
        let old = old.clone();
        self.relink = None;
        let Some(new) = picked else {
            return true;
        };
        if !self.send(Request::Relink(old.clone(), new.clone())) {
            return false;
        }
        self.relinking = Some((old, new));
        true
    }

    /// Brings the screen in line with a worker update. Card lists only apply while
    /// the screen `showing`; preview textures are made in `textures`.
    pub(super) fn apply(
        &mut self,
        update: Update,
        boards: &Boards<'_>,
        textures: &egui::Context,
        showing: bool,
    ) -> Option<Effect> {
        match update {
            Update::Backup { original, path } => {
                if let Some(path) = path {
                    return Some(Effect::OpenBackup { original, path });
                }
                if let Some(card) = self.cards.iter_mut().find(|c| c.path == original) {
                    card.backup_available = false;
                }
                return Some(Effect::BackupUnavailable);
            }
            Update::Removed(path) => {
                self.cards.retain(|card| card.path != path);
                self.preview_order.retain(|p| p != &path);
                self.catalog.dismiss_editor();
                return Some(match self.relinking.take() {
                    Some((old, new)) if old == path => Effect::Moved { old, new },
                    other => {
                        self.relinking = other;
                        Effect::Forgotten(path)
                    }
                });
            }
            Update::Folders(folders) => {
                self.folders = folders
                    .into_iter()
                    .map(|f| Folder {
                        id: f.id.as_u128(),
                        name: f.name,
                        color: f.color,
                    })
                    .collect();
                // A deleted folder leaves no dots on the cards.
                let ids: Vec<u128> = self.folders.iter().map(|f| f.id).collect();
                for card in &mut self.cards {
                    card.folders.retain(|id| ids.contains(id));
                }
                if let View::Folder(id) = self.catalog.view
                    && !ids.contains(&id)
                {
                    self.catalog.view = View::All;
                }
            }
            Update::Meta(path, meta) => {
                if let Some(card) = self.cards.iter_mut().find(|c| c.path == path) {
                    card.pinned = meta.pinned;
                    card.backups = meta.backups;
                    card.tags = meta.tags;
                    card.folders = meta.folders.iter().map(|id| id.as_u128()).collect();
                    card.opened = meta.opened;
                }
            }
            Update::Cards(entries) if showing => {
                // Keep the textures of cards whose file did not change.
                let mut old: HashMap<PathBuf, Card> = std::mem::take(&mut self.cards)
                    .into_iter()
                    .map(|card| (card.path.clone(), card))
                    .collect();
                self.cards = entries
                    .into_iter()
                    .map(|entry| {
                        let previous = old.remove(&entry.path);
                        card(entry, previous, boards)
                    })
                    .collect();
                let kept: std::collections::HashSet<&PathBuf> = self
                    .cards
                    .iter()
                    .filter(|c| !c.previews.is_empty())
                    .map(|c| &c.path)
                    .collect();
                self.preview_order.retain(|path| kept.contains(path));
            }
            Update::Entry(entry) if showing => {
                let index = self.cards.iter().position(|c| c.path == entry.path);
                match index {
                    Some(index) => {
                        let previous = self.cards.swap_remove(index);
                        let replaced = card(entry, Some(previous), boards);
                        if replaced.previews.is_empty() {
                            self.preview_order.retain(|p| p != &replaced.path);
                        }
                        self.cards.push(replaced);
                        let last = self.cards.len() - 1;
                        self.cards.swap(index, last);
                    }
                    None => self.cards.push(card(entry, None, boards)),
                }
            }
            Update::Previews(path, mips) => {
                let card = self.cards.iter_mut().find(|c| c.path == path)?;
                card.loading = false;
                card.previews = mips
                    .into_iter()
                    .enumerate()
                    .map(|(i, mip)| {
                        textures.load_texture(
                            format!("library-{}-{i}", path.display()),
                            egui::ColorImage::from_rgba_unmultiplied(
                                [mip.width as usize, mip.height as usize],
                                &mip.bytes,
                            ),
                            egui::TextureOptions::LINEAR,
                        )
                    })
                    .collect();
                // Bounded: the cards seen longest ago give their textures back.
                self.preview_order.retain(|p| p != &path);
                self.preview_order.push_back(path);
                while self.preview_order.len() > MAX_PREVIEW_CARDS {
                    if let Some(oldest) = self.preview_order.pop_front()
                        && let Some(card) = self.cards.iter_mut().find(|c| c.path == oldest)
                    {
                        card.previews.clear();
                    }
                }
            }
            Update::Cards(_) | Update::Entry(_) => {}
        }
        None
    }
}

/// A library card from the worker's entry, keeping the previews of the card it
/// replaces while the file is unchanged.
fn card(entry: moth_platform::library::Entry, previous: Option<Card>, boards: &Boards<'_>) -> Card {
    let path = entry.path;
    let title = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or(moth_ui::text(moth_ui::Text::Untitled))
        .to_owned();
    let (previews, loading) = match previous {
        Some(card) if card.modified == entry.modified && !entry.missing => {
            (card.previews, card.loading)
        }
        _ => (Vec::new(), false),
    };
    Card {
        unsaved: boards.unsaved.iter().any(|p| *p == path),
        recent: boards.recent.iter().position(|p| p == &path),
        title,
        folders: entry.meta.folders.iter().map(|id| id.as_u128()).collect(),
        opened: entry.meta.opened,
        missing: entry.missing,
        backup_available: entry.backup_available,
        backups: entry.meta.backups,
        known: entry.count.is_some(),
        loading,
        count: entry.count.unwrap_or(0),
        bytes: entry.bytes,
        modified: entry.modified,
        pinned: entry.meta.pinned,
        tags: entry.meta.tags,
        error: entry.error,
        previews,
        path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moth_platform::library::{Entry, Meta};
    use std::time::{Duration, SystemTime};

    fn screen() -> (
        LibraryScreen,
        crossbeam_channel::Receiver<Request>,
        crossbeam_channel::Sender<Result<Update, String>>,
    ) {
        let (requests, asked) = crossbeam_channel::bounded(8);
        let (answers, results) = crossbeam_channel::bounded(8);
        let worker = moth_platform::library::Worker { requests, results };
        (LibraryScreen::new(worker), asked, answers)
    }
    fn entry(name: &str, modified: u64) -> Entry {
        Entry {
            path: PathBuf::from(format!("/boards/{name}.moth")),
            meta: Meta::default(),
            count: Some(1),
            bytes: 10,
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(modified)),
            missing: false,
            backup_available: false,
            error: None,
        }
    }
    fn preview(path: &str) -> Update {
        Update::Previews(
            PathBuf::from(path),
            vec![moth_io::Mip {
                width: 1,
                height: 1,
                bytes: vec![255; 4],
            }],
        )
    }
    const BOARDS: Boards<'static> = Boards {
        recent: &[],
        unsaved: Vec::new(),
    };

    #[test]
    fn updates_keep_previews_of_unchanged_boards_and_drop_the_rest() {
        let (mut screen, _, _) = screen();
        let ctx = egui::Context::default();
        let apply = |screen: &mut LibraryScreen, update| screen.apply(update, &BOARDS, &ctx, true);
        apply(
            &mut screen,
            Update::Cards(vec![entry("a", 1), entry("b", 1)]),
        );
        apply(&mut screen, preview("/boards/a.moth"));
        apply(&mut screen, preview("/boards/b.moth"));
        assert!(screen.cards.iter().all(|c| !c.previews.is_empty()));
        // `a` unchanged, `b` saved since: only `a` keeps its texture.
        apply(
            &mut screen,
            Update::Cards(vec![entry("a", 1), entry("b", 2)]),
        );
        let with = |name: &str| {
            screen
                .cards
                .iter()
                .find(|c| c.title == name)
                .is_some_and(|c| !c.previews.is_empty())
        };
        assert!(with("a") && !with("b"));
        assert_eq!(screen.preview_order, [PathBuf::from("/boards/a.moth")]);
        // One changed entry on its own, in place.
        apply(&mut screen, Update::Entry(entry("a", 3)));
        assert_eq!(screen.cards[0].title, "a");
        assert!(screen.cards[0].previews.is_empty());
        assert!(screen.preview_order.is_empty());
        // A hidden screen ignores card lists and gives every texture back.
        screen.hide();
        apply(&mut screen, preview("/boards/a.moth"));
        assert!(screen.cards.is_empty());
        assert_eq!(
            screen.apply(Update::Cards(vec![entry("a", 1)]), &BOARDS, &ctx, false),
            None
        );
        assert!(screen.cards.is_empty());
    }

    #[test]
    fn preview_textures_stay_within_the_budget() {
        let (mut screen, _, _) = screen();
        let ctx = egui::Context::default();
        let entries = (0..MAX_PREVIEW_CARDS + 5)
            .map(|i| entry(&i.to_string(), 1))
            .collect();
        screen.apply(Update::Cards(entries), &BOARDS, &ctx, true);
        for i in 0..MAX_PREVIEW_CARDS + 5 {
            screen.apply(preview(&format!("/boards/{i}.moth")), &BOARDS, &ctx, true);
        }
        let holding = screen
            .cards
            .iter()
            .filter(|c| !c.previews.is_empty())
            .count();
        assert_eq!(holding, MAX_PREVIEW_CARDS);
        assert!(
            screen.cards[0].previews.is_empty(),
            "the oldest gave its texture back"
        );
        assert!(!screen.cards.last().unwrap().previews.is_empty());
        // A removed board leaves the order too.
        let last = screen.cards.last().unwrap().path.clone();
        let effect = screen.apply(Update::Removed(last.clone()), &BOARDS, &ctx, true);
        assert_eq!(effect, Some(Effect::Forgotten(last.clone())));
        assert!(!screen.preview_order.contains(&last));
    }

    #[test]
    fn find_relinks_through_the_worker_and_moves_the_recent_entry() {
        let (mut screen, asked, _) = screen();
        let ctx = egui::Context::default();
        let (send, answer) = crossbeam_channel::bounded(1);
        let old = PathBuf::from("/boards/old.moth");
        let new = PathBuf::from("/moved/old.moth");
        screen.find(old.clone(), answer);
        assert!(screen.finding());
        assert!(screen.poll_find(), "no answer yet");
        send.send(Some(new.clone())).unwrap();
        assert!(screen.poll_find());
        assert!(!screen.finding());
        assert!(matches!(asked.try_recv(), Ok(Request::Relink(a, b)) if a == old && b == new));
        // The worker drops the old record: the recent list moves, not forgets.
        let effect = screen.apply(Update::Removed(old.clone()), &BOARDS, &ctx, true);
        assert_eq!(effect, Some(Effect::Moved { old, new }));
        // A cancelled dialog sends nothing.
        let (send, answer) = crossbeam_channel::bounded(1);
        screen.find(PathBuf::from("/boards/x.moth"), answer);
        send.send(None).unwrap();
        assert!(screen.poll_find());
        assert!(asked.try_recv().is_err());
    }

    #[test]
    fn deleted_folders_leave_no_dots_and_backups_report_back() {
        let (mut screen, _, _) = screen();
        let ctx = egui::Context::default();
        let kept = moth_scene::Uuid::from_u128(1);
        let gone = moth_scene::Uuid::from_u128(2);
        let mut board = entry("a", 1);
        board.meta.folders = vec![kept, gone];
        screen.apply(Update::Cards(vec![board]), &BOARDS, &ctx, true);
        screen.catalog.view = View::Folder(gone.as_u128());
        let folder = moth_platform::library::Folder {
            id: kept,
            name: "Refs".into(),
            color: 1,
        };
        screen.apply(Update::Folders(vec![folder]), &BOARDS, &ctx, true);
        assert_eq!(screen.cards[0].folders, vec![kept.as_u128()]);
        assert_eq!(screen.catalog.view, View::All);
        let path = screen.cards[0].path.clone();
        let effect = screen.apply(
            Update::Backup {
                original: path.clone(),
                path: None,
            },
            &BOARDS,
            &ctx,
            true,
        );
        assert_eq!(effect, Some(Effect::BackupUnavailable));
        assert!(!screen.cards[0].backup_available);
    }
}
