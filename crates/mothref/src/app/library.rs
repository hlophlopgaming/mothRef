// SPDX-License-Identifier: GPL-3.0-or-later
//! Owns the board index worker, its cards and the catalog UI.
use super::*;

/// Owns the library index and its catalog surface, independent of the active board.
pub(super) struct LibraryScreen {
    pub(super) worker: moth_platform::library::Worker,
    pub(super) cards: Vec<moth_ui::library::Card>,
    pub(super) catalog: moth_ui::library::Library,
    pub(super) refresh: bool,
}

impl LibraryScreen {
    /// Renders the catalog and returns the board-level intents it produced.
    /// Worker send failures are reported through the shared message slot.
    pub(super) fn show(&mut self, ui: &mut egui::Ui, message: &mut String) -> Vec<Intent> {
        let intent = self.catalog.show(ui, &self.cards);
        let mut intents = Vec::new();
        if intent.new {
            intents.push(Intent::NewBoard);
        }
        if intent.browse {
            intents.push(Intent::OpenDialog);
        }
        if let Some(index) = intent.open {
            intents.push(Intent::Open(self.cards[index].path.clone()));
        }
        if let Some((index, tags)) = intent.tags
            && self
                .worker
                .requests
                .try_send(moth_platform::library::Request::Tags(
                    self.cards[index].path.clone(),
                    tags,
                ))
                .is_err()
        {
            *message = moth_ui::text(moth_ui::Text::StorageBusy).into();
        }
        if let Some(index) = intent.pin {
            let card = &self.cards[index];
            if self
                .worker
                .requests
                .try_send(moth_platform::library::Request::Pin(
                    card.path.clone(),
                    !card.pinned,
                ))
                .is_err()
            {
                *message = moth_ui::text(moth_ui::Text::StorageBusy).into();
            }
        }
        intents
    }

    /// Requests a refresh when flagged and drains the index worker into the card list.
    pub(super) fn pump(
        &mut self,
        ctx: &egui::Context,
        message: &mut String,
        recent_paths: &[std::path::PathBuf],
        in_library: bool,
    ) {
        if in_library
            && self.refresh
            && self
                .worker
                .requests
                .try_send(moth_platform::library::Request::Refresh(
                    recent_paths.to_vec(),
                ))
                .is_ok()
        {
            self.refresh = false;
        }
        while let Ok(result) = self.worker.results.try_recv() {
            match result {
                Ok(entry) if in_library => {
                    let path = entry.path;
                    let index = self.cards.iter().position(|c| c.path == path);
                    if index.is_none() && self.cards.len() >= moth_platform::library::MAX_BOARDS {
                        continue;
                    }
                    let title = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or(moth_ui::text(moth_ui::Text::Untitled))
                        .to_owned();
                    let stack = path
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_owned();
                    let previews = entry
                        .previews
                        .into_iter()
                        .enumerate()
                        .map(|(i, mip)| {
                            ctx.load_texture(
                                format!("library-{}-{i}", path.display()),
                                egui::ColorImage::from_rgba_unmultiplied(
                                    [mip.width as usize, mip.height as usize],
                                    &mip.bytes,
                                ),
                                egui::TextureOptions::LINEAR,
                            )
                        })
                        .collect();
                    let card = moth_ui::library::Card {
                        path,
                        title,
                        stack,
                        count: entry.count,
                        bytes: entry.bytes,
                        pinned: entry.pinned,
                        tags: entry.tags,
                        error: entry.error,
                        previews,
                    };
                    if let Some(index) = index {
                        self.cards[index] = card;
                    } else {
                        self.cards.push(card);
                    }
                }
                Err(error) => *message = error,
                _ => {}
            }
        }
    }
}
