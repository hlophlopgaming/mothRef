// SPDX-License-Identifier: GPL-3.0-or-later
//! Catalog layout owns only bounded preview textures, never board content.
use super::{
    icons::{self, Icon},
    *,
};
/// Card metadata is supplied by the indexing worker.
pub struct Card {
    /// Opaque desktop identity; widgets never access the filesystem.
    pub path: std::path::PathBuf,
    /// File stem for navigation.
    pub title: String,
    /// App folders holding the board (`Folder::id`); the file never moves on disk.
    pub folders: Vec<u128>,
    /// Position in the recent list, if the board is on it.
    pub recent: Option<usize>,
    /// When the board was last seen opening, in seconds since the Unix epoch.
    pub opened: u64,
    /// The file is not at its path any more; the card offers to find it.
    pub missing: bool,
    /// The library has read the file at least once, so its count and size are real.
    pub known: bool,
    /// Previews were asked for and have not arrived yet.
    pub loading: bool,
    /// Scene item count.
    pub count: usize,
    /// File size.
    pub bytes: u64,
    /// Last change of the board file.
    pub modified: Option<std::time::SystemTime>,
    /// A recovery journal from an earlier session holds changes not saved to the file.
    pub unsaved: bool,
    /// Catalog pin.
    pub pinned: bool,
    /// Durable catalog annotations.
    pub tags: Vec<String>,
    /// Failed files remain accessible for error inspection.
    pub error: Option<String>,
    /// At most four thumbnails per card, freed when the catalog closes.
    pub previews: Vec<egui::TextureHandle>,
}
/// An app folder as the sidebar shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Folder {
    /// Stable identity.
    pub id: u128,
    /// Name in the sidebar.
    pub name: String,
    /// Index into the theme's group colors.
    pub color: u8,
}
/// Which boards the library shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// Every board the library knows.
    #[default]
    All,
    /// The recent list.
    Recent,
    /// Pinned boards.
    Pinned,
    /// One app folder.
    Folder(u128),
}
/// A change to the app folders, carried out by the library worker.
#[derive(Clone, Debug, PartialEq)]
pub enum FolderChange {
    /// Make a folder with this name.
    Create(String),
    /// Rename a folder.
    Rename(u128, String),
    /// Change a folder's color.
    Recolor(u128, u8),
    /// Delete a folder; its boards stay.
    Delete(u128),
}
/// The drag payload of a board card, dropped onto a sidebar folder.
struct CardDrag(usize);
/// Search and view preferences stay bounded independently of the number of boards.
#[derive(Default)]
pub struct Library {
    /// Which boards are shown.
    pub view: View,
    /// Bounded search query.
    pub search: String,
    /// Accessible list view alternative.
    pub list: bool,
    /// Card order; the default keeps the recent-file order.
    pub order: Order,
    /// Active tag collection.
    pub tag: Option<String>,
    editing: Option<usize>,
    edit_tags: String,
    trashing: Option<usize>,
    /// The folder name being typed: for a new folder (`None`) or a rename.
    naming: Option<(Option<u128>, String)>,
}
/// How the cards are ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Order {
    /// Most recently opened first, as the recent-file list has them.
    #[default]
    LastOpened,
    /// Most recently changed file first.
    LastEdited,
    /// Alphabetical, ignoring case.
    Name,
    /// Most items first.
    Items,
    /// Largest file first.
    Size,
}
impl Order {
    const ALL: [Self; 5] = [
        Self::LastOpened,
        Self::LastEdited,
        Self::Name,
        Self::Items,
        Self::Size,
    ];
    fn label(self) -> &'static str {
        text(match self {
            Self::LastOpened => Text::LastOpened,
            Self::LastEdited => Text::LastEdited,
            Self::Name => Text::Name,
            Self::Items => Text::OrderItems,
            Self::Size => Text::OrderSize,
        })
    }
    /// Sorts card indices; ties keep the recent-file order.
    fn sort(self, cards: &[Card], indices: &mut [usize]) {
        match self {
            Self::LastOpened => indices.sort_by_key(|&i| std::cmp::Reverse(cards[i].opened)),
            Self::LastEdited => {
                indices.sort_by_key(|&i| std::cmp::Reverse(cards[i].modified));
            }
            Self::Name => indices.sort_by_cached_key(|&i| cards[i].title.to_lowercase()),
            Self::Items => indices.sort_by_key(|&i| std::cmp::Reverse(cards[i].count)),
            Self::Size => indices.sort_by_key(|&i| std::cmp::Reverse(cards[i].bytes)),
        }
    }
}
/// Catalog intent leaves file operations to the platform worker.
#[derive(Default)]
pub struct Intent {
    /// Open this actual catalog entry.
    pub open: Option<usize>,
    /// Persist a pin change.
    pub pin: Option<usize>,
    /// Create an empty board.
    pub new: bool,
    /// Browse for an existing board.
    pub browse: bool,
    /// Persist bounded tags on the indexing worker.
    pub tags: Option<(usize, String)>,
    /// Show the board file in the system file manager.
    pub reveal: Option<usize>,
    /// Take the board off the list; the file stays.
    pub forget: Option<usize>,
    /// Move the board file to the system trash (already confirmed).
    pub trash: Option<usize>,
    /// Create, rename, recolor or delete an app folder.
    pub folder: Option<FolderChange>,
    /// Put a board into a folder (`true`) or take it out (`false`).
    pub file: Option<(usize, u128, bool)>,
    /// Ask where a missing board file is now.
    pub relink: Option<usize>,
    /// Cards on screen whose previews are not loaded or asked for yet.
    pub visible: Vec<std::path::PathBuf>,
}
impl Library {
    /// Closing the panel must not reattach an old draft to a different catalog entry.
    pub fn dismiss_editor(&mut self) {
        self.editing = None;
        self.edit_tags.clear();
        self.trashing = None;
        self.naming = None;
    }
    /// The actions of one card, shared by its right-click menu and its "⋯" button.
    fn card_menu(
        &mut self,
        ui: &mut egui::Ui,
        card: &Card,
        index: usize,
        folders: &[Folder],
        intent: &mut Intent,
    ) {
        if card.missing {
            if ui.button(text(Text::FindFile)).clicked() {
                intent.relink = Some(index);
            }
        } else if ui.button(text(Text::Open)).clicked() {
            intent.open = Some(index);
        }
        if ui
            .button(text(if card.pinned {
                Text::UnpinBoard
            } else {
                Text::PinBoard
            }))
            .clicked()
        {
            intent.pin = Some(index);
        }
        if ui.button(text(Text::EditTags)).clicked() {
            self.editing = Some(index);
            self.edit_tags = card.tags.join(", ");
        }
        icons::submenu(ui, text(Text::AddToFolder), |ui| {
            for folder in folders {
                let mut inside = card.folders.contains(&folder.id);
                if ui.checkbox(&mut inside, &folder.name).changed() {
                    intent.file = Some((index, folder.id, inside));
                }
            }
            if !folders.is_empty() {
                ui.separator();
            }
            if ui.button(text(Text::NewFolder)).clicked() {
                self.naming = Some((None, String::new()));
            }
        });
        if !card.missing && ui.button(text(Text::ShowInFolder)).clicked() {
            intent.reveal = Some(index);
        }
        ui.separator();
        if let View::Folder(folder) = self.view
            && ui.button(text(Text::RemoveFromFolder)).clicked()
        {
            intent.file = Some((index, folder, false));
        }
        if ui.button(text(Text::RemoveFromList)).clicked() {
            intent.forget = Some(index);
        }
        if !card.missing
            && ui
                .button(
                    egui::RichText::new(text(Text::MoveToTrash))
                        .color(crate::tokens(ui.ctx()).danger),
                )
                .clicked()
        {
            self.trashing = Some(index);
        }
    }
    /// One sidebar row: icon or color dot, label, count; returns the label's response.
    fn side_row(
        ui: &mut egui::Ui,
        mark: Result<Icon, egui::Color32>,
        label: &str,
        selected: bool,
        count: usize,
    ) -> egui::Response {
        ui.horizontal(|ui| {
            let tokens = crate::tokens(ui.ctx());
            let area = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(20., 30.));
            match mark {
                Ok(icon) => icons::paint(ui, area, icon, tokens.text_muted),
                Err(color) => {
                    ui.painter().circle_filled(area.center(), 5., color);
                }
            }
            ui.add_space(tokens.space_xl as f32);
            let response = ui.add(egui::Button::selectable(selected, label).truncate());
            side_count(ui, count);
            response
        })
        .inner
    }
    fn sidebar(
        &mut self,
        ui: &mut egui::Ui,
        cards: &[Card],
        folders: &[Folder],
        tags: &[&String],
        intent: &mut Intent,
    ) {
        let tokens = crate::tokens(ui.ctx());
        self.search(ui);
        ui.add_space(tokens.space_m as f32);
        for (icon, label, view, count) in [
            (Icon::Grid, Text::AllBoards, View::All, cards.len()),
            (
                Icon::Clock,
                Text::Recent,
                View::Recent,
                cards.iter().filter(|c| c.recent.is_some()).count(),
            ),
            (
                Icon::Pin,
                Text::Pinned,
                View::Pinned,
                cards.iter().filter(|c| c.pinned).count(),
            ),
        ] {
            if Self::side_row(
                ui,
                Ok(icon),
                text(label),
                self.view == view && self.tag.is_none(),
                count,
            )
            .clicked()
            {
                self.view = view;
                self.tag = None;
            }
        }
        ui.add_space(tokens.space_section as f32);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(text(Text::Folders).to_uppercase())
                    .small()
                    .strong()
                    .color(tokens.text_muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons::button(ui, Icon::Plus, text(Text::NewFolder), false).clicked() {
                    self.naming = Some((None, String::new()));
                }
            });
        });
        if folders.is_empty() {
            ui.label(
                egui::RichText::new(text(Text::FoldersHint))
                    .small()
                    .color(tokens.text_muted),
            );
        }
        for folder in folders {
            let color = tokens.groups[folder.color as usize % tokens.groups.len()];
            let response = Self::side_row(
                ui,
                Err(color),
                &folder.name,
                self.view == View::Folder(folder.id) && self.tag.is_none(),
                cards
                    .iter()
                    .filter(|c| c.folders.contains(&folder.id))
                    .count(),
            );
            // A dragged card lights the folder up and lands in it on release.
            let row = response
                .rect
                .expand2(egui::vec2(tokens.space_xl as f32 * 2., 2.));
            if response.dnd_hover_payload::<CardDrag>().is_some() {
                ui.painter().rect_stroke(
                    row,
                    tokens.radius_m as f32,
                    egui::Stroke::new(1.5, tokens.accent),
                    egui::StrokeKind::Inside,
                );
            }
            if let Some(card) = response.dnd_release_payload::<CardDrag>() {
                intent.file = Some((card.0, folder.id, true));
            }
            if response.clicked() {
                self.view = View::Folder(folder.id);
                self.tag = None;
            }
            response.context_menu(|ui| {
                if ui.button(text(Text::RenameFolder)).clicked() {
                    self.naming = Some((Some(folder.id), folder.name.clone()));
                }
                ui.horizontal(|ui| {
                    for (index, color) in tokens.groups.iter().enumerate() {
                        if icons::swatch(
                            ui,
                            [color.r(), color.g(), color.b()],
                            index == folder.color as usize,
                        )
                        .clicked()
                        {
                            intent.folder = Some(FolderChange::Recolor(folder.id, index as u8));
                        }
                    }
                });
                ui.separator();
                if ui
                    .button(egui::RichText::new(text(Text::DeleteFolder)).color(tokens.danger))
                    .on_hover_text(text(Text::DeleteFolderHint))
                    .clicked()
                {
                    intent.folder = Some(FolderChange::Delete(folder.id));
                    if self.view == View::Folder(folder.id) {
                        self.view = View::All;
                    }
                }
            });
        }
        ui.add_space(tokens.space_xl as f32);
        ui.label(
            egui::RichText::new(text(Text::SmartCollections).to_uppercase())
                .small()
                .strong()
                .color(tokens.text_muted),
        );
        for tag in tags.iter().take(12) {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.tag.as_ref() == Some(*tag), format!("#{tag}"))
                    .clicked()
                {
                    self.tag = Some((*tag).clone());
                    self.view = View::All;
                }
                side_count(ui, cards.iter().filter(|c| c.tags.contains(tag)).count());
            });
        }
        ui.add_space(tokens.space_l as f32);
    }
    /// Displays real recent boards, with responsive grid and touch-friendly navigation.
    pub fn show(&mut self, ui: &mut egui::Ui, cards: &[Card], folders: &[Folder]) -> Intent {
        let mut intent = Intent::default();
        let mut tags: Vec<_> = cards.iter().flat_map(|c| c.tags.iter()).collect();
        tags.sort();
        tags.dedup();
        if ui.available_width() > 700. {
            egui::Panel::left("library_sidebar")
                .exact_size((ui.available_width() * 0.22).clamp(160., 280.))
                .frame(
                    egui::Frame::new()
                        .fill(ui.visuals().panel_fill)
                        .inner_margin(crate::tokens(ui.ctx()).space_l as i8),
                )
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.sidebar(ui, cards, folders, &tags, &mut intent);
                    });
                });
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(crate::tokens(ui.ctx()).sidebar)
                    .inner_margin(if ui.available_width() > 700. {
                        crate::tokens(ui.ctx()).space_section as i8
                    } else {
                        crate::tokens(ui.ctx()).space_header as i8
                    }),
            )
            .show(ui, |ui| {
                let needle = self.search.to_lowercase();
                let mut indices: Vec<usize> = cards
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| {
                        let in_view = match self.view {
                            View::All => true,
                            View::Recent => c.recent.is_some(),
                            View::Pinned => c.pinned,
                            View::Folder(id) => c.folders.contains(&id),
                        };
                        in_view
                            && self.tag.as_ref().is_none_or(|tag| c.tags.contains(tag))
                            && (c.title.to_lowercase().contains(&needle)
                                || c.tags.iter().any(|tag| tag.contains(&needle)))
                    })
                    .map(|(i, _)| i)
                    .collect();
                if self.view == View::Recent && self.order == Order::LastOpened {
                    indices.sort_by_key(|&i| cards[i].recent);
                } else {
                    self.order.sort(cards, &mut indices);
                }
                ui.horizontal_wrapped(|ui| {
                    ui.heading(
                        self.tag.as_deref().unwrap_or(match self.view {
                            View::All => text(Text::AllBoards),
                            View::Recent => text(Text::Recent),
                            View::Pinned => text(Text::Pinned),
                            View::Folder(id) => folders
                                .iter()
                                .find(|f| f.id == id)
                                .map_or(text(Text::AllBoards), |f| f.name.as_str()),
                        }),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        intent.browse = ui.button(text(Text::OpenFile)).clicked();
                        intent.new = ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("+  {}", text(Text::NewBoard)))
                                        .color(crate::tokens(ui.ctx()).on_accent),
                                )
                                .fill(crate::tokens(ui.ctx()).accent),
                            )
                            .clicked();
                        if icons::button(ui, Icon::List, text(Text::ListView), self.list).clicked()
                        {
                            self.list = true;
                        }
                        if icons::button(ui, Icon::Grid, text(Text::GridView), !self.list).clicked()
                        {
                            self.list = false;
                        }
                        egui::ComboBox::from_id_salt("library_order")
                            .selected_text(self.order.label())
                            .show_ui(ui, |ui| {
                                for order in Order::ALL {
                                    ui.selectable_value(&mut self.order, order, order.label());
                                }
                            });
                    });
                });
                ui.label(
                    egui::RichText::new(format!(
                        "{}  ·  {}",
                        crate::counted(
                            crate::locale(),
                            indices.len(),
                            [Text::BoardOne, Text::BoardFew, Text::BoardMany]
                        ),
                        size(indices.iter().map(|&i| cards[i].bytes).sum::<u64>())
                    ))
                    .small()
                    .color(crate::tokens(ui.ctx()).text_muted),
                );
                ui.add_space(crate::tokens(ui.ctx()).space_l as f32);
                if ui.available_width() <= 700. {
                    self.search(ui);
                    ui.horizontal_wrapped(|ui| {
                        let mut views = vec![
                            (View::All, text(Text::AllBoards).to_owned()),
                            (View::Recent, text(Text::Recent).to_owned()),
                            (View::Pinned, text(Text::Pinned).to_owned()),
                        ];
                        views.extend(folders.iter().map(|f| (View::Folder(f.id), f.name.clone())));
                        for (view, label) in views {
                            if ui.selectable_label(self.view == view, label).clicked() {
                                self.view = view;
                            }
                        }
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .selectable_label(self.tag.is_none(), text(Text::All))
                        .clicked()
                    {
                        self.tag = None;
                    }
                    for tag in tags.iter().take(12) {
                        if ui
                            .selectable_label(self.tag.as_ref() == Some(*tag), format!("#{tag}"))
                            .clicked()
                        {
                            self.tag = Some((*tag).clone());
                        }
                    }
                });
                ui.add_space(crate::tokens(ui.ctx()).space_m as f32);
                if indices.is_empty() {
                    ui.add_space(crate::tokens(ui.ctx()).space_empty as f32);
                    ui.vertical_centered(|ui| {
                        ui.label(text(if cards.is_empty() {
                            Text::LibraryEmpty
                        } else if matches!(self.view, View::Folder(_)) && self.search.is_empty() {
                            Text::FolderEmpty
                        } else {
                            Text::NoMatches
                        }));
                    });
                }
                // Only the rows on screen are laid out, so a library of thousands of
                // boards costs what the visible cards cost.
                let scrollbar = ui.spacing().scroll.allocated_width();
                let room = (ui.available_width() - scrollbar).max(1.);
                let columns = if self.list {
                    1
                } else {
                    ((room + 24.) / 240.).floor().max(1.) as usize
                };
                let card_height = if self.list { 90. } else { 270. };
                let rows: Vec<&[usize]> = indices.chunks(columns).collect();
                let pitch = card_height
                    + crate::tokens(ui.ctx()).space_l as f32
                    + ui.spacing().item_spacing.y;
                egui::ScrollArea::vertical().show_rows(ui, pitch, rows.len(), |ui, range| {
                    let width =
                        (ui.available_width() - (columns - 1) as f32 * 24.) / columns as f32;
                    for row in &rows[range] {
                        intent.visible.extend(
                            row.iter()
                                .filter(|&&i| {
                                    cards[i].previews.is_empty()
                                        && !cards[i].loading
                                        && !cards[i].missing
                                })
                                .map(|&i| cards[i].path.clone()),
                        );
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x =
                                crate::tokens(ui.ctx()).space_xl as f32;
                            for &index in row.iter() {
                                let c = &cards[index];
                                ui.allocate_ui_with_layout(
                                    egui::vec2(width, card_height),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        let preview_height = if self.list { 50. } else { 170. };
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(width, preview_height),
                                            egui::Sense::click_and_drag(),
                                        );
                                        response.dnd_set_drag_payload(CardDrag(index));
                                        let lift = ui.ctx().animate_bool_with_time(
                                            response.id.with("lift"),
                                            response.hovered(),
                                            ui.style().animation_time,
                                        );
                                        let rect = egui::Rect::from_center_size(
                                            rect.center() - egui::vec2(0., lift * 2.),
                                            rect.size() * (1. + lift * 0.01),
                                        );
                                        ui.painter().rect_filled(
                                            rect.translate(egui::vec2(0., 3. * lift)),
                                            crate::tokens(ui.ctx()).radius_card as f32,
                                            crate::tokens(ui.ctx())
                                                .shadow
                                                .gamma_multiply(0.2 * lift),
                                        );
                                        ui.painter().rect_filled(
                                            rect,
                                            crate::tokens(ui.ctx()).radius_card as f32,
                                            crate::tokens(ui.ctx()).canvas,
                                        );
                                        if c.previews.is_empty() {
                                            icons::paint(
                                                ui,
                                                rect,
                                                Icon::Image,
                                                crate::tokens(ui.ctx()).text_muted,
                                            );
                                        }
                                        for (i, texture) in c.previews.iter().enumerate() {
                                            let area = if i == 0 {
                                                egui::Rect::from_min_max(
                                                    rect.min + egui::vec2(12., 12.),
                                                    egui::pos2(
                                                        rect.center().x - 4.,
                                                        rect.bottom() - 12.,
                                                    ),
                                                )
                                            } else {
                                                let h = (rect.height()
                                                    - 24.
                                                    - 8. * (c.previews.len().saturating_sub(2))
                                                        as f32)
                                                    / (c.previews.len() - 1) as f32;
                                                egui::Rect::from_min_size(
                                                    egui::pos2(
                                                        rect.center().x + 4.,
                                                        rect.top()
                                                            + 12.
                                                            + (i - 1) as f32 * (h + 8.),
                                                    ),
                                                    egui::vec2(rect.width() / 2. - 16., h),
                                                )
                                            };
                                            let size = texture.size_vec2();
                                            let fitted = size
                                                * (area.width() / size.x)
                                                    .min(area.height() / size.y);
                                            ui.painter().image(
                                                texture.id(),
                                                egui::Rect::from_center_size(area.center(), fitted),
                                                egui::Rect::from_min_max(
                                                    egui::Pos2::ZERO,
                                                    egui::pos2(1., 1.),
                                                ),
                                                crate::tokens(ui.ctx()).image_tint,
                                            );
                                        }
                                        if lift > 0. {
                                            ui.painter().rect_stroke(
                                                rect,
                                                crate::tokens(ui.ctx()).radius_card as f32,
                                                egui::Stroke::new(
                                                    1.,
                                                    crate::tokens(ui.ctx())
                                                        .accent
                                                        .gamma_multiply(lift),
                                                ),
                                                egui::StrokeKind::Inside,
                                            );
                                        }
                                        if c.missing {
                                            badge(
                                                ui,
                                                rect,
                                                Text::FileMissing,
                                                Text::FileMissingHint,
                                            );
                                        } else if c.unsaved {
                                            badge(ui, rect, Text::UnsavedChip, Text::UnsavedHint);
                                        }
                                        folder_dots(ui, rect, c, folders);
                                        if response.clicked() {
                                            if c.missing {
                                                intent.relink = Some(index);
                                            } else {
                                                intent.open = Some(index);
                                            }
                                        }
                                        let response = if c.missing {
                                            response.on_hover_text(c.path.display().to_string())
                                        } else if let Some(error) = &c.error {
                                            response.on_hover_text(error)
                                        } else {
                                            response
                                        };
                                        response.context_menu(|ui| {
                                            self.card_menu(ui, c, index, folders, &mut intent);
                                        });
                                        ui.horizontal(|ui| {
                                            if ui
                                                .add(
                                                    egui::Label::new(
                                                        egui::RichText::new(&c.title).strong(),
                                                    )
                                                    .sense(egui::Sense::click())
                                                    .truncate(),
                                                )
                                                .clicked()
                                            {
                                                if c.missing {
                                                    intent.relink = Some(index);
                                                } else {
                                                    intent.open = Some(index);
                                                }
                                            }
                                            if icons::button(
                                                ui,
                                                Icon::Pin,
                                                text(Text::Pinned),
                                                c.pinned,
                                            )
                                            .clicked()
                                            {
                                                intent.pin = Some(index);
                                            }
                                            let more = icons::button(
                                                ui,
                                                Icon::More,
                                                text(Text::More),
                                                false,
                                            );
                                            egui::Popup::menu(&more).show(|ui| {
                                                self.card_menu(ui, c, index, folders, &mut intent);
                                            });
                                        });
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(card_details(c, self.list))
                                                    .small()
                                                    .color(crate::tokens(ui.ctx()).text_muted),
                                            )
                                            .truncate(),
                                        );
                                        ui.horizontal_wrapped(|ui| {
                                            if ui.small_button(text(Text::Tags)).clicked() {
                                                self.editing = Some(index);
                                                self.edit_tags = c.tags.join(", ");
                                            }
                                            for tag in c.tags.iter().take(2) {
                                                ui.label(
                                                    egui::RichText::new(format!("#{tag}"))
                                                        .size(10.)
                                                        .color(crate::tokens(ui.ctx()).text_muted),
                                                );
                                            }
                                        });
                                    },
                                );
                            }
                        });
                        ui.add_space(crate::tokens(ui.ctx()).space_l as f32);
                    }
                });
            });
        if let Some(index) = self.editing.filter(|&i| i < cards.len()) {
            let tokens = crate::tokens(ui.ctx());
            egui::Modal::new(egui::Id::new("catalog_tags"))
                .frame(
                    egui::Frame::popup(&ui.ctx().global_style())
                        .corner_radius(tokens.radius_l)
                        .inner_margin(egui::Margin::same(tokens.space_xl as i8)),
                )
                .show(ui.ctx(), |ui| {
                    ui.set_width(
                        (tokens.space_xl as f32 * 15.)
                            .min(ui.ctx().content_rect().width() - tokens.space_xl as f32 * 4.)
                            .max(1.),
                    );
                    icons::dialog_title(ui, text(Text::EditTags));
                    ui.label(egui::RichText::new(&cards[index].title).color(tokens.text_muted));
                    ui.add_space(tokens.space_s as f32);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.edit_tags)
                            .char_limit(264)
                            .hint_text(text(Text::TagsHint))
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(tokens.space_l as f32);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(text(Text::Apply)).color(tokens.on_accent),
                                )
                                .fill(tokens.accent),
                            )
                            .clicked()
                        {
                            intent.tags = Some((index, self.edit_tags.clone()));
                            self.editing = None;
                        }
                        if ui.button(text(Text::Cancel)).clicked() {
                            self.editing = None;
                        }
                    });
                });
        }
        self.naming_dialog(ui.ctx(), &mut intent);
        if let Some(CardDrag(index)) = egui::DragAndDrop::payload::<CardDrag>(ui.ctx()).as_deref()
            && let (Some(card), Some(pointer)) = (cards.get(*index), ui.ctx().pointer_latest_pos())
        {
            // The dragged card's title follows the pointer to the folder it is dropped on.
            let tokens = crate::tokens(ui.ctx());
            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("library_drag"),
            ));
            let galley = painter.layout_no_wrap(
                card.title.clone(),
                egui::TextStyle::Body.resolve(ui.style()),
                tokens.text,
            );
            let rect = egui::Rect::from_min_size(
                pointer + egui::vec2(14., 10.),
                galley.size() + egui::vec2(16., 8.),
            );
            painter.rect_filled(rect, tokens.radius_m as f32, tokens.surface_raised);
            painter.rect_stroke(
                rect,
                tokens.radius_m as f32,
                egui::Stroke::new(1., tokens.border),
                egui::StrokeKind::Inside,
            );
            painter.galley(rect.min + egui::vec2(8., 4.), galley, tokens.text);
        }
        if let Some(index) = self.trashing.filter(|&i| i < cards.len()) {
            let tokens = crate::tokens(ui.ctx());
            egui::Modal::new(egui::Id::new("catalog_trash"))
                .frame(
                    egui::Frame::popup(&ui.ctx().global_style())
                        .corner_radius(tokens.radius_l)
                        .inner_margin(egui::Margin::same(tokens.space_xl as i8)),
                )
                .show(ui.ctx(), |ui| {
                    ui.set_width(
                        (tokens.space_xl as f32 * 15.)
                            .min(ui.ctx().content_rect().width() - tokens.space_xl as f32 * 4.)
                            .max(1.),
                    );
                    icons::dialog_title(
                        ui,
                        &text(Text::TrashQuestion).replace("{name}", &cards[index].title),
                    );
                    ui.label(egui::RichText::new(text(Text::TrashBody)).color(tokens.text_muted));
                    ui.add_space(tokens.space_l as f32);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // The danger fill is checked for contrast against the surface color.
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(text(Text::TrashConfirm))
                                        .color(tokens.surface),
                                )
                                .fill(tokens.danger),
                            )
                            .clicked()
                        {
                            intent.trash = Some(index);
                            self.trashing = None;
                        }
                        if ui.button(text(Text::Cancel)).clicked() {
                            self.trashing = None;
                        }
                    });
                });
        } else {
            self.trashing = None;
        }
        intent
    }
    /// "New folder" and "Rename folder": a name field with Cancel and Create / Rename.
    fn naming_dialog(&mut self, ctx: &egui::Context, intent: &mut Intent) {
        let Some((folder, name)) = &mut self.naming else {
            return;
        };
        let tokens = crate::tokens(ctx);
        let mut close = false;
        egui::Modal::new(egui::Id::new("library_folder_name"))
            .frame(
                egui::Frame::popup(&ctx.global_style())
                    .corner_radius(tokens.radius_l)
                    .inner_margin(egui::Margin::same(tokens.space_xl as i8)),
            )
            .show(ctx, |ui| {
                ui.set_width(
                    (tokens.space_xl as f32 * 13.)
                        .min(ctx.content_rect().width() - tokens.space_xl as f32 * 4.)
                        .max(1.),
                );
                icons::dialog_title(
                    ui,
                    text(if folder.is_some() {
                        Text::RenameFolderTitle
                    } else {
                        Text::NewFolder
                    }),
                );
                ui.add_space(tokens.space_s as f32);
                let field = ui.add(
                    egui::TextEdit::singleline(name)
                        .char_limit(64)
                        .hint_text(text(Text::FolderNameHint))
                        .desired_width(f32::INFINITY),
                );
                if !field.has_focus() && !field.lost_focus() {
                    field.request_focus();
                }
                let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(tokens.space_l as f32);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ready = !name.trim().is_empty();
                    let confirm = ui.add_enabled(
                        ready,
                        egui::Button::new(
                            egui::RichText::new(text(if folder.is_some() {
                                Text::RenameButton
                            } else {
                                Text::CreateFolder
                            }))
                            .color(tokens.on_accent),
                        )
                        .fill(tokens.accent),
                    );
                    if ready && (confirm.clicked() || submitted) {
                        intent.folder = Some(match folder {
                            Some(id) => FolderChange::Rename(*id, name.trim().to_owned()),
                            None => FolderChange::Create(name.trim().to_owned()),
                        });
                        close = true;
                    }
                    if ui.button(text(Text::Cancel)).clicked() {
                        close = true;
                    }
                });
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });
        if close {
            self.naming = None;
        }
    }
    fn search(&mut self, ui: &mut egui::Ui) {
        // A rounded field with a search icon, not a bare strip.
        let tokens = crate::tokens(ui.ctx());
        egui::Frame::new()
            .fill(tokens.surface_raised)
            .stroke(egui::Stroke::new(1., tokens.border))
            .corner_radius(tokens.radius_m)
            .inner_margin(egui::Margin::symmetric(
                tokens.space_s as i8,
                tokens.space_xs as i8,
            ))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (icon, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(16.), egui::Sense::hover());
                    crate::icons::paint(ui, icon, crate::icons::Icon::Search, tokens.text_muted);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text(text(Text::Search))
                            .frame(egui::Frame::NONE)
                            .char_limit(256)
                            .desired_width(f32::INFINITY),
                    );
                });
            });
    }
}

/// The muted count at the right end of a sidebar row.
fn side_count(ui: &mut egui::Ui, count: usize) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(
            egui::RichText::new(count.to_string())
                .small()
                .color(crate::tokens(ui.ctx()).text_muted),
        );
    });
}
/// A file size the way file managers show it: "740 KB", "4.2 MB".
fn size(bytes: u64) -> String {
    if bytes < 1_000_000 {
        format!("{} KB", bytes.div_ceil(1000))
    } else {
        format!("{:.1} MB", bytes as f64 / 1_000_000.)
    }
}
/// "11 items · 4.2 MB · 2 h ago", plus the file's folder on disk in the list view.
fn card_details(card: &Card, list: bool) -> String {
    if card.missing {
        return card.path.display().to_string();
    }
    if !card.known {
        return "…".into();
    }
    let mut parts = vec![crate::counted(
        crate::locale(),
        card.count,
        [Text::ItemOne, Text::ItemFew, Text::ItemMany],
    )];
    parts.push(size(card.bytes));
    if let Some(modified) = card.modified {
        parts.push(crate::ago(modified));
    }
    if list && let Some(parent) = card.path.parent() {
        parts.push(parent.display().to_string());
    }
    parts.join("  ·  ")
}
/// A chip in the preview's corner: "Not saved" for a pending recovery journal, or
/// "File not found" for a board that moved; its hint explains it.
fn badge(ui: &mut egui::Ui, preview: egui::Rect, label: Text, hint: Text) {
    let tokens = crate::tokens(ui.ctx());
    let font = egui::TextStyle::Small.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(text(label).to_owned(), font, tokens.danger);
    let rect = egui::Rect::from_min_size(
        preview.min + egui::vec2(8., 8.),
        galley.size() + egui::vec2(12., 6.),
    );
    ui.painter()
        .rect_filled(rect, tokens.radius_m as f32, tokens.surface_raised);
    ui.painter()
        .galley(rect.min + egui::vec2(6., 3.), galley, tokens.danger);
    ui.interact(
        rect,
        ui.id()
            .with(("badge", preview.min.x as i32, preview.min.y as i32)),
        egui::Sense::hover(),
    )
    .on_hover_text(text(hint));
}
/// One dot per app folder holding the board, in the folder's color, at the preview's
/// top-right corner.
fn folder_dots(ui: &egui::Ui, preview: egui::Rect, card: &Card, folders: &[Folder]) {
    let tokens = crate::tokens(ui.ctx());
    let colors = folders
        .iter()
        .filter(|f| card.folders.contains(&f.id))
        .map(|f| tokens.groups[f.color as usize % tokens.groups.len()]);
    for (i, color) in colors.take(6).enumerate() {
        let center = egui::pos2(preview.right() - 14. - i as f32 * 14., preview.top() + 14.);
        ui.painter().circle_filled(center, 5., color);
        ui.painter()
            .circle_stroke(center, 5., egui::Stroke::new(1.5, tokens.canvas));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orders_sort_by_their_own_field_and_keep_recent_order_on_ties() {
        let now = std::time::SystemTime::now();
        let card = |title: &str, count, bytes, age: u64| Card {
            path: format!("{title}.moth").into(),
            title: title.into(),
            folders: vec![],
            recent: None,
            opened: 1000 - age,
            missing: false,
            known: true,
            loading: false,
            count,
            bytes,
            modified: Some(now - std::time::Duration::from_secs(age)),
            unsaved: false,
            pinned: false,
            tags: vec![],
            error: None,
            previews: vec![],
        };
        let cards = [
            card("beta", 3, 10, 50),
            card("Alpha", 9, 5, 10),
            card("gamma", 3, 99, 900),
        ];
        let sorted = |order: Order| {
            let mut indices = vec![0, 1, 2];
            order.sort(&cards, &mut indices);
            indices
        };
        assert_eq!(sorted(Order::LastOpened), [1, 0, 2]);
        assert_eq!(sorted(Order::LastEdited), [1, 0, 2]);
        assert_eq!(sorted(Order::Name), [1, 0, 2]);
        assert_eq!(sorted(Order::Items), [1, 0, 2]);
        assert_eq!(sorted(Order::Size), [2, 0, 1]);
        assert_eq!(size(740_000), "740 KB");
        assert_eq!(size(4_200_000), "4.2 MB");
    }
    #[test]
    fn card_labels_flow_below_previews() {
        let ctx = egui::Context::default();
        theme(&ctx, &Tokens::DARK);
        let cards: Vec<_> = (0..8)
            .map(|i| Card {
                path: format!("board-{i}.moth").into(),
                title: format!("Reference {i}"),
                folders: vec![],
                recent: Some(i),
                opened: 0,
                missing: false,
                known: true,
                loading: false,
                count: 11,
                bytes: 100,
                modified: None,
                unsaved: false,
                pinned: false,
                tags: vec![],
                error: None,
                previews: vec![],
            })
            .collect();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440., 900.),
                )),
                ..Default::default()
            },
            |ui| {
                Library::default().show(ui, &cards, &[]);
            },
        );
        output.textures_delta.clear();
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::epaint::Shape::Text(text) = &shape.shape {
                    text.galley.text().starts_with("Reference ").then_some(text)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(labels.len(), 8);
        assert!(
            labels[0].pos.y > 200.,
            "title must follow the preview vertically"
        );
        assert!(labels.iter().all(|text| text.galley.size().x > 65. && text.pos.x + text.galley.size().x < 1440.));
    }
}
