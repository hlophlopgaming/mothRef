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
    /// User-selected alphabetical order; default preserves recent-file order.
    pub alphabetical: bool,
    /// Active tag collection.
    pub tag: Option<String>,
    editing: Option<usize>,
    edit_tags: String,
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
}
impl Library {
    /// Closing the panel must not reattach an old draft to a different catalog entry.
    pub fn dismiss_editor(&mut self) {
        self.editing = None;
        self.edit_tags.clear();
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
                        .inner_margin(16),
                )
                .show(ui, |ui| {
                    self.search(ui);
                    ui.add_space(12.);
                    for (icon, label, pinned) in [
                        (Icon::Grid, Text::AllBoards, false),
                        (Icon::Pin, Text::Pinned, true),
                    ] {
                        ui.horizontal(|ui| {
                            icons::paint(
                                ui,
                                egui::Rect::from_min_size(ui.cursor().min, egui::vec2(20., 30.)),
                                icon,
                                MUTED,
                            );
                            ui.add_space(24.);
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
                        });
                    }
                    ui.add_space(28.);
                    ui.label(
                        egui::RichText::new(text(Text::Stacks))
                            .monospace()
                            .size(10.)
                            .color(MUTED),
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
                                ACCENT,
                            );
                            ui.add_space(24.);
                            if ui
                                .selectable_label(self.stack.as_deref() == Some(stack), stack)
                                .clicked()
                            {
                                self.stack = Some(stack.into());
                                self.pinned = false;
                            }
                        });
                    }
                    ui.add_space(24.);
                    ui.label(
                        egui::RichText::new(text(Text::SmartCollections))
                            .monospace()
                            .size(10.)
                            .color(MUTED),
                    );
                    for tag in tags.iter().take(12) {
                        if ui
                            .selectable_label(self.tag.as_ref() == Some(*tag), format!("#{tag}"))
                            .clicked()
                        {
                            self.tag = Some((*tag).clone());
                            self.stack = None;
                            self.pinned = false;
                        }
                    }
                    ui.add_space(16.);
                    intent.browse = ui.button(text(Text::Open)).clicked();
                });
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(23, 25, 30))
                    .inner_margin(if ui.available_width() > 700. { 28 } else { 14 }),
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
                        intent.new = ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("+  {}", text(Text::NewBoard)))
                                        .color(BACKGROUND),
                                )
                                .fill(ACCENT),
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
                            .selected_text(if self.alphabetical {
                                text(Text::Name)
                            } else {
                                text(Text::LastOpened)
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.alphabetical,
                                    false,
                                    text(Text::LastOpened),
                                );
                                ui.selectable_value(&mut self.alphabetical, true, text(Text::Name));
                            });
                    });
                });
                ui.label(
                    egui::RichText::new(format!(
                        "{} {}  ·  {:.1} MB",
                        cards.len(),
                        text(Text::Boards),
                        cards.iter().map(|c| c.bytes).sum::<u64>() as f64 / 1_000_000.
                    ))
                    .monospace()
                    .size(11.)
                    .color(MUTED),
                );
                ui.add_space(16.);
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
                        intent.browse = ui.button(text(Text::Open)).clicked();
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
                ui.add_space(12.);
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
                if self.alphabetical {
                    indices.sort_by(|&a, &b| cards[a].title.cmp(&cards[b].title));
                }
                if indices.is_empty() {
                    ui.add_space(80.);
                    ui.vertical_centered(|ui| {
                        ui.label(text(if cards.is_empty() {
                            Text::LibraryEmpty
                        } else {
                            Text::NoMatches
                        }));
                        intent.browse |= ui.button(text(Text::Open)).clicked();
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
                            ui.spacing_mut().item_spacing.x = 24.;
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
                                            10.,
                                            Color32::BLACK.gamma_multiply(0.2 * lift),
                                        );
                                        ui.painter().rect_filled(rect, 10., BACKGROUND);
                                        if c.previews.is_empty() {
                                            icons::paint(ui, rect, Icon::Image, MUTED);
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
                                                Color32::WHITE,
                                            );
                                        }
                                        if lift > 0. {
                                            ui.painter().rect_stroke(
                                                rect,
                                                10.,
                                                egui::Stroke::new(1., ACCENT.gamma_multiply(lift)),
                                                egui::StrokeKind::Inside,
                                            );
                                        }
                                        if response.clicked() {
                                            intent.open = Some(index);
                                        }
                                        if let Some(error) = &c.error {
                                            response.on_hover_text(error);
                                        }
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
                                        });
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{} {}  ·  {}",
                                                c.count,
                                                text(Text::Items),
                                                c.stack
                                            ))
                                            .monospace()
                                            .size(10.)
                                            .color(MUTED),
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
                                                        .color(MUTED),
                                                );
                                            }
                                        });
                                    },
                                );
                            }
                        });
                        ui.add_space(16.);
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
        intent
    }
    fn search(&mut self, ui: &mut egui::Ui) {
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text(text(Text::Search))
                .char_limit(256)
                .desired_width(f32::INFINITY),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn card_labels_flow_below_previews() {
        let ctx = egui::Context::default();
        theme(&ctx);
        let cards: Vec<_> = (0..8)
            .map(|i| Card {
                path: format!("board-{i}.moth").into(),
                title: format!("Reference {i}"),
                stack: "Characters".into(),
                count: 11,
                bytes: 100,
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
