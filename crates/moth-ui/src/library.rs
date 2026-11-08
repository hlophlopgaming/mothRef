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
    /// Parent folder supplies an actual stack name.
    pub stack: String,
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
/// Search and view preferences stay bounded independently of the number of boards.
#[derive(Default)]
pub struct Library {
    /// All or pinned collection.
    pub pinned: bool,
    /// Folder filter.
    pub stack: Option<String>,
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
            Self::LastOpened => {}
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
}
impl Library {
    /// Closing the panel must not reattach an old draft to a different catalog entry.
    pub fn dismiss_editor(&mut self) {
        self.editing = None;
        self.edit_tags.clear();
        self.trashing = None;
    }
    /// The actions of one card, shared by its right-click menu and its "⋯" button.
    fn card_menu(&mut self, ui: &mut egui::Ui, card: &Card, index: usize, intent: &mut Intent) {
        if ui.button(text(Text::Open)).clicked() {
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
        if ui.button(text(Text::ShowInFolder)).clicked() {
            intent.reveal = Some(index);
        }
        ui.separator();
        if ui.button(text(Text::RemoveFromList)).clicked() {
            intent.forget = Some(index);
        }
        if ui
            .button(
                egui::RichText::new(text(Text::MoveToTrash)).color(crate::tokens(ui.ctx()).danger),
            )
            .clicked()
        {
            self.trashing = Some(index);
        }
    }
    /// Displays real recent boards, with responsive grid and touch-friendly navigation.
    pub fn show(&mut self, ui: &mut egui::Ui, cards: &[Card]) -> Intent {
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
                    self.search(ui);
                    ui.add_space(crate::tokens(ui.ctx()).space_m as f32);
                    let pinned_count = cards.iter().filter(|c| c.pinned).count();
                    for (icon, label, pinned, count) in [
                        (Icon::Grid, Text::AllBoards, false, cards.len()),
                        (Icon::Pin, Text::Pinned, true, pinned_count),
                    ] {
                        ui.horizontal(|ui| {
                            icons::paint(
                                ui,
                                egui::Rect::from_min_size(ui.cursor().min, egui::vec2(20., 30.)),
                                icon,
                                crate::tokens(ui.ctx()).text_muted,
                            );
                            ui.add_space(crate::tokens(ui.ctx()).space_xl as f32);
                            if ui
                                .selectable_label(
                                    self.pinned == pinned
                                        && self.stack.is_none()
                                        && self.tag.is_none(),
                                    text(label),
                                )
                                .clicked()
                            {
                                self.pinned = pinned;
                                self.stack = None;
                                self.tag = None;
                            }
                            side_count(ui, count);
                        });
                    }
                    ui.add_space(crate::tokens(ui.ctx()).space_section as f32);
                    ui.label(
                        egui::RichText::new(text(Text::Stacks).to_uppercase())
                            .small()
                            .strong()
                            .color(crate::tokens(ui.ctx()).text_muted),
                    );
                    let mut stacks: Vec<&str> = cards
                        .iter()
                        .map(|c| c.stack.as_str())
                        .filter(|s| !s.is_empty())
                        .collect();
                    stacks.sort_unstable();
                    stacks.dedup();
                    for stack in stacks {
                        ui.horizontal(|ui| {
                            icons::paint(
                                ui,
                                egui::Rect::from_min_size(ui.cursor().min, egui::vec2(20., 30.)),
                                Icon::Folder,
                                crate::tokens(ui.ctx()).accent_text,
                            );
                            ui.add_space(crate::tokens(ui.ctx()).space_xl as f32);
                            if ui
                                .selectable_label(self.stack.as_deref() == Some(stack), stack)
                                .clicked()
                            {
                                self.stack = Some(stack.into());
                                self.pinned = false;
                            }
                            side_count(ui, cards.iter().filter(|c| c.stack == stack).count());
                        });
                    }
                    ui.add_space(crate::tokens(ui.ctx()).space_xl as f32);
                    ui.label(
                        egui::RichText::new(text(Text::SmartCollections).to_uppercase())
                            .small()
                            .strong()
                            .color(crate::tokens(ui.ctx()).text_muted),
                    );
                    for tag in tags.iter().take(12) {
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    self.tag.as_ref() == Some(*tag),
                                    format!("#{tag}"),
                                )
                                .clicked()
                            {
                                self.tag = Some((*tag).clone());
                                self.stack = None;
                                self.pinned = false;
                            }
                            side_count(ui, cards.iter().filter(|c| c.tags.contains(tag)).count());
                        });
                    }
                    ui.add_space(crate::tokens(ui.ctx()).space_l as f32);
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
                ui.horizontal_wrapped(|ui| {
                    ui.heading(self.stack.as_deref().or(self.tag.as_deref()).unwrap_or(
                        if self.pinned {
                            text(Text::Pinned)
                        } else {
                            text(Text::AllBoards)
                        },
                    ));
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
                            cards.len(),
                            [Text::BoardOne, Text::BoardFew, Text::BoardMany]
                        ),
                        size(cards.iter().map(|c| c.bytes).sum::<u64>())
                    ))
                    .small()
                    .color(crate::tokens(ui.ctx()).text_muted),
                );
                ui.add_space(crate::tokens(ui.ctx()).space_l as f32);
                if ui.available_width() <= 700. {
                    self.search(ui);
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(!self.pinned, text(Text::AllBoards))
                            .clicked()
                        {
                            self.pinned = false;
                            self.stack = None;
                        }
                        if ui
                            .selectable_label(self.pinned, text(Text::Pinned))
                            .clicked()
                        {
                            self.pinned = true;
                            self.stack = None;
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
                let needle = self.search.to_lowercase();
                let mut indices: Vec<usize> = cards
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| {
                        (!self.pinned || c.pinned)
                            && self.tag.as_ref().is_none_or(|tag| c.tags.contains(tag))
                            && self.stack.as_ref().is_none_or(|s| s == &c.stack)
                            && (c.title.to_lowercase().contains(&needle)
                                || c.stack.to_lowercase().contains(&needle)
                                || c.tags.iter().any(|tag| tag.contains(&needle)))
                    })
                    .map(|(i, _)| i)
                    .collect();
                self.order.sort(cards, &mut indices);
                if indices.is_empty() {
                    ui.add_space(crate::tokens(ui.ctx()).space_empty as f32);
                    ui.vertical_centered(|ui| {
                        ui.label(text(if cards.is_empty() {
                            Text::LibraryEmpty
                        } else {
                            Text::NoMatches
                        }));
                    });
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let columns = if self.list {
                        1
                    } else {
                        ((ui.available_width() + 24.) / 240.).floor().max(1.) as usize
                    };
                    let width =
                        (ui.available_width() - (columns - 1) as f32 * 24.) / columns as f32;
                    for row in indices.chunks(columns) {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x =
                                crate::tokens(ui.ctx()).space_xl as f32;
                            for &index in row {
                                let c = &cards[index];
                                ui.allocate_ui_with_layout(
                                    egui::vec2(width, if self.list { 90. } else { 270. }),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        let preview_height = if self.list { 50. } else { 170. };
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(width, preview_height),
                                            egui::Sense::click(),
                                        );
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
                                        if c.unsaved {
                                            unsaved_badge(ui, rect);
                                        }
                                        if response.clicked() {
                                            intent.open = Some(index);
                                        }
                                        let response = if let Some(error) = &c.error {
                                            response.on_hover_text(error)
                                        } else {
                                            response
                                        };
                                        response.context_menu(|ui| {
                                            self.card_menu(ui, c, index, &mut intent);
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
                                                intent.open = Some(index);
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
                                                self.card_menu(ui, c, index, &mut intent);
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
            egui::Modal::new(egui::Id::new("catalog_tags")).show(ui.ctx(), |ui| {
                ui.set_width(ui.ctx().content_rect().width().min(400.) - 40.);
                ui.heading(text(Text::EditTags));
                ui.label(&cards[index].title);
                ui.add(
                    egui::TextEdit::singleline(&mut self.edit_tags)
                        .char_limit(264)
                        .hint_text(text(Text::TagsHint))
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    if ui.button(text(Text::Cancel)).clicked() {
                        self.editing = None;
                    }
                    if ui.button(text(Text::Apply)).clicked() {
                        intent.tags = Some((index, self.edit_tags.clone()));
                        self.editing = None;
                    }
                });
            });
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
/// "11 items · 4.2 MB · 2 h ago", plus the stack in the list view where there is room for it.
fn card_details(card: &Card, list: bool) -> String {
    let mut parts = vec![crate::counted(
        crate::locale(),
        card.count,
        [Text::ItemOne, Text::ItemFew, Text::ItemMany],
    )];
    parts.push(size(card.bytes));
    if let Some(modified) = card.modified {
        parts.push(crate::ago(modified));
    }
    if list && !card.stack.is_empty() {
        parts.push(card.stack.clone());
    }
    parts.join("  ·  ")
}
/// A "Not saved" chip in the preview's corner for a board with a pending recovery journal.
fn unsaved_badge(ui: &mut egui::Ui, preview: egui::Rect) {
    let tokens = crate::tokens(ui.ctx());
    let font = egui::TextStyle::Small.resolve(ui.style());
    let galley =
        ui.painter()
            .layout_no_wrap(text(Text::UnsavedChip).to_owned(), font, tokens.danger);
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
            .with(("unsaved", preview.min.x as i32, preview.min.y as i32)),
        egui::Sense::hover(),
    )
    .on_hover_text(text(Text::UnsavedHint));
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
            stack: String::new(),
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
        assert_eq!(sorted(Order::LastOpened), [0, 1, 2]);
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
                stack: "Characters".into(),
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
                Library::default().show(ui, &cards);
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
