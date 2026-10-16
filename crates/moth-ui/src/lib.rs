// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Shared theme and localized M0 controls.
use egui::{Color32, Context};
/// Centralized strings keep widgets independent of the selected locale.
#[derive(Clone, Copy)]
pub enum Text {
    /// Shared localization key for Title.
    Title,
    /// Shared localization key for Paste.
    Paste,
    /// Shared localization key for Fit.
    Fit,
    /// Shared localization key for Pan.
    Pan,
    /// Shared localization key for Select.
    Select,
    /// Shared localization key for Empty.
    Empty,
    /// Shared localization key for Volatile.
    Volatile,
    /// Shared localization key for Close.
    Close,
    /// Shared localization key for Keep.
    Keep,
    /// Shared localization key for CloseWarning.
    CloseWarning,
    /// Shared localization key for BoardFull.
    BoardFull,
    /// Shared localization key for SourceChanged.
    SourceChanged,
}
/// English is the initial locale; translation lookup has one entry point.
pub fn text(key: Text) -> &'static str {
    match key {
        Text::SourceChanged => "The source file changed. Its previous preview cannot be reloaded.",
        Text::BoardFull => "M0 supports up to 10,000 images per board",
        Text::Title => "MothRef",
        Text::Paste => "Paste image",
        Text::Fit => "Fit board",
        Text::Pan => "Pan",
        Text::Select => "Select",
        Text::Empty => "Drop images here, or paste an image to begin.",
        Text::Volatile => "M0 preview • not saved • PNG / JPEG • up to 2048 px",
        Text::Close => "Discard and close",
        Text::Keep => "Keep working",
        Text::CloseWarning => "This board is only in memory. Closing discards its layout.",
    }
}
/// Color tokens are shared by the canvas overlays and the egui theme.
pub const BACKGROUND: Color32 = Color32::from_rgb(22, 24, 29);
/// Selection color remains visible over both dark and bright images.
pub const ACCENT: Color32 = Color32::from_rgb(181, 206, 143);
/// Applies one house theme without embedding colors in individual controls.
pub fn theme(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        "Hack".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../fonts/Hack-Regular.ttf"
        ))),
    );
    fonts
        .families
        .insert(egui::FontFamily::Proportional, vec!["Hack".into()]);
    fonts
        .families
        .insert(egui::FontFamily::Monospace, vec!["Hack".into()]);
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = Color32::from_rgb(31, 34, 40);
    style.visuals.selection.bg_fill = ACCENT;
    style.spacing.interact_size = egui::vec2(44., 44.);
    style.animation_time = 0.12;
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(egui::Theme::Dark);
}
/// Toolbar actions also provide touch paths for keyboard operations.
#[derive(Default)]
pub struct Actions {
    /// Requests worker-side clipboard access.
    pub paste: bool,
    /// Requests camera framing without mutating the board.
    pub fit: bool,
    /// Keeps camera scale available to rendering and input.
    pub zoom: f32,
}
/// State is metadata only, with no egui texture ownership.
pub struct Status<'a> {
    /// Makes hardware and software rendering distinguishable.
    pub adapter: &'a str,
    /// Communicates the board size without exposing scene storage.
    pub count: usize,
    /// Communicates selection without retaining item data.
    pub selected: usize,
    /// Keeps camera scale available to rendering and input.
    pub zoom: f32,
    /// Reports allocated GPU pages rather than source image sizes.
    pub texture_bytes: u64,
    /// Surfaces worker errors without blocking a frame.
    pub message: &'a str,
}
/// Returns user intent, leaving scene changes to the command layer.
pub fn toolbar(ui: &mut egui::Ui, status: Status<'_>, pan: &mut bool) -> Actions {
    let mut actions = Actions::default();
    egui::Panel::top("toolbar").show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong(text(Text::Title));
            actions.paste = ui.button(text(Text::Paste)).clicked();
            actions.fit = ui.button(text(Text::Fit)).clicked();
            ui.selectable_value(pan, true, text(Text::Pan));
            ui.selectable_value(pan, false, text(Text::Select));
            if ui.button("−").clicked() {
                actions.zoom = 0.8;
            }
            if ui.button("+").clicked() {
                actions.zoom = 1.25;
            }
            ui.label(format!(
                "{:.0}% · {} / {} · {:.1} MiB",
                status.zoom * 100.,
                status.selected,
                status.count,
                status.texture_bytes as f64 / 1048576.
            ));
        });
        ui.small(status.adapter);
        ui.small(text(Text::Volatile));
        if !status.message.is_empty() {
            ui.colored_label(ACCENT, status.message);
        }
    });
    actions
}
