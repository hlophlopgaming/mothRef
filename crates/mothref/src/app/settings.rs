// SPDX-License-Identifier: GPL-3.0-or-later
//! Preference schema drives persistence, rows and command search.
use super::{Intent, bindings::Action};
use moth_ui::{Text, text};
use std::collections::BTreeMap;
const MIN_VRAM_MIB: u32 = 256;
const MAX_VRAM_MIB: u32 = 1 << 20;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    General,
    Appearance,
    Canvas,
    Adding,
    Window,
    Performance,
    Keys,
    Integrations,
    Developers,
}
impl Section {
    pub const ALL: [Self; 9] = [
        Self::General,
        Self::Appearance,
        Self::Canvas,
        Self::Adding,
        Self::Window,
        Self::Performance,
        Self::Keys,
        Self::Integrations,
        Self::Developers,
    ];
    pub fn label(self) -> Text {
        match self {
            Self::General => Text::GeneralSettings,
            Self::Appearance => Text::AppearanceSettings,
            Self::Canvas => Text::CanvasSettings,
            Self::Adding => Text::AddingSettings,
            Self::Window => Text::WindowSettings,
            Self::Performance => Text::PerformanceSettings,
            Self::Keys => Text::KeysSettings,
            Self::Integrations => Text::IntegrationsSettings,
            Self::Developers => Text::DeveloperSettings,
        }
    }
    fn icon(self) -> moth_ui::icons::Icon {
        use moth_ui::icons::Icon;
        match self {
            Self::General => Icon::Settings,
            Self::Appearance => Icon::Palette,
            Self::Canvas => Icon::Pan,
            Self::Adding => Icon::ImagePlus,
            Self::Window => Icon::Maximize,
            Self::Performance => Icon::Gauge,
            Self::Keys => Icon::Keyboard,
            Self::Integrations => Icon::Plug,
            Self::Developers => Icon::Code,
        }
    }
}
pub(super) enum Kind {
    Toggle,
    Color,
    Language,
    VideoLimit,
    Number(f64, f64, &'static str),
    Count(u64, u64),
    Choice(&'static [Text]),
}
pub(super) struct Entry {
    pub key: &'static str,
    pub section: Section,
    pub label: Text,
    pub kind: Kind,
    pub hint: Option<Text>,
    get: fn(&Settings) -> String,
    set: fn(&mut Settings, &BTreeMap<String, String>),
}
macro_rules! schema {
    ($($field:ident: $ty:ty = $default:expr, $key:literal, $section:ident, $label:ident, $kind:expr, $get:expr, $decode:expr;)+) => {
        #[derive(Clone)]
        pub(super) struct Settings { $(pub $field: $ty,)+ }
        impl Default for Settings { fn default() -> Self { Self { $($field: $default,)+ } } }
        pub(super) const SCHEMA: &[Entry] = &[$(Entry {
            key:$key, section:Section::$section, label:Text::$label, kind:$kind, hint:None,
            get: $get, set: |settings, values| settings.$field = ($decode)(values),
        },)+];
    }
}
schema! {
    view_controls_right: bool = true, "view_controls_right", Window, ViewControlsRight, Kind::Toggle, |s| s.view_controls_right.to_string(), |values: &BTreeMap<String,String>| values.get("view_controls_right").and_then(|v|v.parse().ok()).unwrap_or(true);
    over_hint: bool = true, "over_hint", Window, OverHintSetting, Kind::Toggle, |s| s.over_hint.to_string(), |values: &BTreeMap<String,String>| values.get("over_hint").and_then(|v|v.parse().ok()).unwrap_or(true);
    global_shortcuts: bool = false, "global_shortcuts", Window, GlobalShortcuts, Kind::Toggle, |s| s.global_shortcuts.to_string(), |values: &BTreeMap<String,String>| values.get("global_shortcuts").and_then(|v|v.parse().ok()).unwrap_or(false);
    disable_click_through_on_activate: bool = true, "disable_click_through_on_activate", Window, DisableClickThroughOnActivate, Kind::Toggle, |s| s.disable_click_through_on_activate.to_string(), |values: &BTreeMap<String,String>| values.get("disable_click_through_on_activate").and_then(|v|v.parse().ok()).unwrap_or(true);
    background_color: Option<[u8; 3]> = None, "background_color", Appearance, BackgroundColor, Kind::Color, |s| moth_ui::theme::encode_color(s.background_color), |values: &BTreeMap<String,String>| values.get("background_color").and_then(|v| moth_ui::theme::decode_color(v));
    canvas_color: Option<[u8; 3]> = None, "canvas_color", Appearance, CanvasColor, Kind::Color, |s| moth_ui::theme::encode_color(s.canvas_color), |values: &BTreeMap<String,String>| values.get("canvas_color").and_then(|v| moth_ui::theme::decode_color(v));
    grayscale_tint: Option<[u8; 3]> = None, "grayscale_tint", Appearance, GrayscaleTint, Kind::Color, |s| moth_ui::theme::encode_color(s.grayscale_tint), |values: &BTreeMap<String,String>| values.get("grayscale_tint").and_then(|v| moth_ui::theme::decode_color(v));
    selection_color: Option<[u8; 3]> = None, "selection_color", Appearance, SelectionColor, Kind::Color, |s| moth_ui::theme::encode_color(s.selection_color), |values: &BTreeMap<String,String>| values.get("selection_color").and_then(|v| moth_ui::theme::decode_color(v));
    lock_maximized: bool = true, "lock_maximized", Window, LockWhileMaximized, Kind::Toggle, |s| s.lock_maximized.to_string(), |values: &BTreeMap<String,String>| values.get("lock_maximized").and_then(|v|v.parse().ok()).unwrap_or(true);

    language: moth_ui::Locale = moth_ui::locale(), "language", General, Language, Kind::Language, |s| s.language.code().to_owned(), |values: &BTreeMap<String,String>| values.get("language").and_then(|v|moth_ui::Locale::from_code(v)).unwrap_or_else(moth_ui::locale);

    swap_pen_buttons: bool = false, "swap_pen_buttons", Keys, SwapPenButtons, Kind::Toggle, |s| s.swap_pen_buttons.to_string(), |values: &BTreeMap<String,String>| values.get("swap_pen_buttons").and_then(|v|v.parse().ok()).unwrap_or(false);
    shortcut_feedback: bool = true, "shortcut_feedback", Keys, ShortcutFeedback, Kind::Toggle, |s| s.shortcut_feedback.to_string(), |values: &BTreeMap<String,String>| values
            .get("shortcut_feedback")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
    reduced_motion: bool = false, "reduced_motion", Appearance, ReducedMotion, Kind::Toggle, |s| s.reduced_motion.to_string(), |values: &BTreeMap<String,String>| values
            .get(moth_platform::settings::REDUCED_MOTION)
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
    show_outlines: bool = true, "show_outlines", Canvas, ShowOutlines, Kind::Toggle, |s| s.show_outlines.to_string(), |values: &BTreeMap<String,String>| values
            .get("show_outlines")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
    drag_animation: bool = true, "drag_animation", Canvas, DragAnimation, Kind::Toggle, |s| s.drag_animation.to_string(), |values: &BTreeMap<String,String>| values
            .get("drag_animation")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
    drag_opacity: f32 = super::motion::DEFAULT_DRAG_OPACITY, "drag_opacity", Canvas, DragOpacity, Kind::Number(0.1,1., ""), |s| s.drag_opacity.to_string(), |values: &BTreeMap<String,String>| values
            .get("drag_opacity")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && (0.1..=1.).contains(v))
            .unwrap_or(super::motion::DEFAULT_DRAG_OPACITY);
    wheel: u8 = 0, "wheel", Canvas, WheelZoom, Kind::Choice(&[Text::WheelZoom,Text::WheelPan,Text::WheelOff]), |s| s.wheel.to_string(), |values: &BTreeMap<String,String>| values
            .get("wheel")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v < 3)
            .unwrap_or(0);
    sampling: u8 = 0, "sampling", Canvas, ImageSampling, Kind::Choice(&[Text::SamplingSmooth,Text::SamplingNearest,Text::SamplingAutomatic]), |s| s.sampling.to_string(), |values: &BTreeMap<String,String>| values.get("sampling").and_then(|v| v.parse().ok()).filter(|v| *v < 3).unwrap_or(0);
    smooth_zoom: bool = true, "smooth_zoom", Canvas, SmoothZoom, Kind::Toggle, |s| s.smooth_zoom.to_string(), |values: &BTreeMap<String,String>| values
            .get("smooth_zoom")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
    zoom_increment: f32 = 10., "zoom_increment", Canvas, ZoomIncrement, Kind::Number(1.,100., "%"), |s| s.zoom_increment.to_string(), |values: &BTreeMap<String,String>| values
            .get("zoom_increment")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && (1.0..=100.0).contains(v))
            .unwrap_or(10.);
    show_minimap: bool = true, "show_minimap", Canvas, ShowMinimap, Kind::Toggle, |s| s.show_minimap.to_string(), |values: &BTreeMap<String,String>| values.get("show_minimap").and_then(|v| v.parse().ok()).unwrap_or(true);
    minimap_images: bool = true, "minimap_images", Canvas, MinimapImages, Kind::Toggle, |s| s.minimap_images.to_string(), |values: &BTreeMap<String,String>| values.get("minimap_images").and_then(|v| v.parse().ok()).unwrap_or(true);
    minimap_opacity: f32 = 100., "minimap_opacity", Canvas, MinimapOpacity, Kind::Number(0.,100., "%"), |s| s.minimap_opacity.to_string(), |values: &BTreeMap<String,String>| values.get("minimap_opacity").and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite() && (0.0..=100.0).contains(v)).unwrap_or(100.);
    minimap_distance: f32 = 100., "minimap_distance", Canvas, MinimapDistance, Kind::Number(0.,100., "%"), |s| s.minimap_distance.to_string(), |values: &BTreeMap<String,String>| values.get("minimap_distance").and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite() && (0.0..=100.0).contains(v)).unwrap_or(100.);
    minimap_position: u8 = 6, "minimap_position", Canvas, MinimapPosition, Kind::Choice(&[Text::PositionTopLeft, Text::PositionTopCenter, Text::PositionTopRight, Text::PositionMiddleLeft, Text::PositionCenter, Text::PositionMiddleRight, Text::PositionBottomLeft, Text::PositionBottomCenter, Text::PositionBottomRight]), |s| s.minimap_position.to_string(), |values: &BTreeMap<String,String>| values.get("minimap_position").and_then(|v| v.parse().ok()).filter(|v| *v < 9).unwrap_or(6);
    minimap_size: f32 = moth_ui::Tokens::LIGHT.minimap_max, "minimap_size", Canvas, MinimapSize, Kind::Number(120.,600., ""), |s| s.minimap_size.to_string(), |values: &BTreeMap<String,String>| values.get("minimap_size").and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite() && (120.0..=600.0).contains(v)).unwrap_or(moth_ui::Tokens::LIGHT.minimap_max);
    keyboard_pan_mode: u8 = 1, "keyboard_pan_mode", Canvas, KeyboardPanMode, Kind::Choice(&[Text::PanStepped, Text::PanFlying]), |s| s.keyboard_pan_mode.to_string(), |values: &BTreeMap<String,String>| values.get("keyboard_pan_mode").and_then(|v| v.parse().ok()).filter(|v| *v < 2).unwrap_or(1);
    keyboard_pan_step: f32 = 200., "keyboard_pan_step", Canvas, KeyboardPanStep, Kind::Number(1.,2000., ""), |s| s.keyboard_pan_step.to_string(), |values: &BTreeMap<String,String>| values.get("keyboard_pan_step").and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite() && (1.0..=2000.0).contains(v)).unwrap_or(200.);
    keyboard_pan_speed: f32 = 800., "keyboard_pan_speed", Canvas, KeyboardPanSpeed, Kind::Number(10.,5000., ""), |s| s.keyboard_pan_speed.to_string(), |values: &BTreeMap<String,String>| values.get("keyboard_pan_speed").and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite() && (10.0..=5000.0).contains(v)).unwrap_or(800.);
    rotation_snap: bool = false, "rotation_snap", Canvas, SnapRotation, Kind::Toggle, |s| s.rotation_snap.to_string(), |values: &BTreeMap<String,String>| values
            .get("rotation_snap")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
    scale_with_window: bool = false, "scale_with_window", Canvas, ScaleWithWindow, Kind::Toggle, |s| s.scale_with_window.to_string(), |values: &BTreeMap<String,String>| values
            .get("scale_with_window")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
    autosave: u8 = 3, "autosave", General, Autosave, Kind::Choice(&[Text::AutosaveOff, Text::Autosave15s, Text::Autosave30s, Text::Autosave1m, Text::Autosave2m, Text::Autosave3m, Text::Autosave5m]), |s| s.autosave.to_string(), |values: &BTreeMap<String,String>| values.get("autosave").and_then(|v| v.parse().ok()).filter(|v| *v < 7).unwrap_or(3);
    undo_limit: usize = 256, "undo_limit", General, UndoLimit, Kind::Count(0,4096), |s| s.undo_limit.to_string(), |values: &BTreeMap<String,String>| values
            .get("undo_limit")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v <= 4096)
            .unwrap_or(256);
    vram_limit_mib: u32 = 0, "vram_limit_mib", Performance, CacheLimit, Kind::VideoLimit, |s| s.vram_limit_mib.to_string(), |values: &BTreeMap<String,String>| values
            .get("vram_limit_mib")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v == 0 || (MIN_VRAM_MIB..=MAX_VRAM_MIB).contains(v))
            .unwrap_or(0);
    performance_mode: moth_io::CacheMode = moth_io::CacheMode::default(), "performance_mode", Performance, PerformanceMode, Kind::Choice(&[Text::ModeFull,Text::ModeMemory,Text::ModeMemoryLow,Text::ModeDisk,Text::ModeNoCache]), |s| s.performance_mode.code().to_owned(), |values: &BTreeMap<String,String>| values
            .get("performance_mode")
            .and_then(|v| moth_io::CacheMode::from_code(v.as_str()))
            .unwrap_or_default();
    bake_mode: moth_io::BakeMode = moth_io::BakeMode::default(), "bake_mode", Performance, BakeTiles, Kind::Choice(&[Text::BakeVisible,Text::BakeBoard,Text::BakeOff]), |s| s.bake_mode.code().to_owned(), |values: &BTreeMap<String,String>| values
            .get("bake_mode")
            .and_then(|v| moth_io::BakeMode::from_code(v.as_str()))
            .unwrap_or_default();
    dark: bool = false, "dark", Appearance, DarkTheme, Kind::Toggle, |s| s.dark.to_string(), |values: &BTreeMap<String,String>| values
            .get("dark")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
    ui_scale: f32 = 1., "ui_scale", Appearance, UiScale, Kind::Number(0.75,2., ""), |s| s.ui_scale.to_string(), |values: &BTreeMap<String,String>| values
            .get("ui_scale")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && (0.75..=2.).contains(v))
            .unwrap_or(1.);
    gap: f32 = 24., "arrange_gap", Adding, ImageGap, Kind::Number(1.,10000., ""), |s| s.gap.to_string(), |values: &BTreeMap<String,String>| values
            .get("arrange_gap")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && (1.0..=10000.).contains(v))
            .unwrap_or(24.);
    threshold: usize = 5, "arrange_prompt_threshold", Adding, ManyImages, Kind::Count(0,10000), |s| s.threshold.to_string(), |values: &BTreeMap<String,String>| values
            .get("arrange_prompt_threshold")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v <= 10000)
            .unwrap_or(5);
    auto_arrange: bool = false, "auto_arrange", Adding, AutoArrange, Kind::Toggle, |s| s.auto_arrange.to_string(), |values: &BTreeMap<String,String>| values
            .get("auto_arrange")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
    default_arrange: u8 = 0, "default_arrange", Adding, NewImageLayout, Kind::Choice(&[Text::Horizontal,Text::Vertical,Text::Grid,Text::Optimal]), |s| s.default_arrange.to_string(), |values: &BTreeMap<String,String>| values
            .get("default_arrange")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v < 4)
            .unwrap_or(0);
    arrange_choice: u8 = 1, "arrange_choice", Developers, PackingChoice, Kind::Choice(&[Text::PackBestArea,Text::PackBestShortSide,Text::PackBestLongSide,Text::PackWorstArea,Text::PackWorstShortSide,Text::PackWorstLongSide]), |s| s.arrange_choice.to_string(), |values: &BTreeMap<String,String>| values
            .get("arrange_choice")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v < 6)
            .unwrap_or(1);
    arrange_split: u8 = 0, "arrange_split", Developers, PackingSplit, Kind::Choice(&[Text::SplitShorterLeftover,Text::SplitLongerLeftover,Text::SplitMinimizeArea,Text::SplitMaximizeArea,Text::SplitShorterAxis,Text::SplitLongerAxis]), |s| s.arrange_split.to_string(), |values: &BTreeMap<String,String>| values
            .get("arrange_split")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v < 6)
            .unwrap_or(0);
}
impl Settings {
    /// How long a board may stay unsaved before it saves itself; `None` is off.
    pub fn autosave_interval(&self) -> Option<std::time::Duration> {
        const SECONDS: [u64; 7] = [0, 15, 30, 60, 120, 180, 300];
        SECONDS
            .get(usize::from(self.autosave))
            .filter(|&&seconds| seconds > 0)
            .map(|&seconds| std::time::Duration::from_secs(seconds))
    }
    pub fn tokens(&self) -> moth_ui::Tokens {
        let mut tokens = if self.dark {
            moth_ui::Tokens::DARK
        } else {
            moth_ui::Tokens::LIGHT
        };
        tokens.custom_colors(
            self.background_color,
            self.canvas_color,
            self.selection_color,
        );
        tokens
    }
    pub fn preferences(&self) -> Vec<(&'static str, String)> {
        SCHEMA
            .iter()
            .map(|entry| (entry.key, (entry.get)(self)))
            .collect()
    }
    pub fn minimap_alignment(&self) -> egui::Align2 {
        use egui::Align2;
        match self.minimap_position {
            0 => Align2::LEFT_TOP,
            1 => Align2::CENTER_TOP,
            2 => Align2::RIGHT_TOP,
            3 => Align2::LEFT_CENTER,
            4 => Align2::CENTER_CENTER,
            5 => Align2::RIGHT_CENTER,
            7 => Align2::CENTER_BOTTOM,
            8 => Align2::RIGHT_BOTTOM,
            _ => Align2::LEFT_BOTTOM,
        }
    }
    pub fn load(&mut self, values: &BTreeMap<String, String>) {
        for entry in SCHEMA {
            (entry.set)(self, values);
        }
    }
}
/// Read-only snapshots shown under Settings › For developers, refreshed by the app.
#[derive(Default)]
pub(super) struct Diagnostics {
    /// Detected adapter memory; `None` until detection reports.
    pub video: Option<moth_platform::gpu::VideoMemory>,
    pub disk_cache_usage: Option<Result<u64, String>>,
    pub disk_cache_busy: bool,
    /// "Clear disk cache" was asked for and not yet started.
    pub clear_disk_cache: bool,
    pub bake_pending: usize,
}
impl Diagnostics {
    /// The video memory limit: the user's override, else what the adapter allows.
    pub fn vram_limit(&self, settings: &Settings) -> Option<u64> {
        if settings.vram_limit_mib > 0 {
            Some(u64::from(settings.vram_limit_mib) * 1024 * 1024)
        } else {
            self.video.as_ref().map(|v| v.limit)
        }
    }
}
#[derive(Default)]
pub(super) struct SettingsWindow {
    pub open: bool,
    pub section: Option<Section>,
    query: String,
    pub focus_row: Option<usize>,
}
impl SettingsWindow {
    pub fn open_row(&mut self, index: usize) {
        if let Some(entry) = SCHEMA.get(index) {
            self.section = Some(entry.section);
            self.focus_row = Some(index);
            self.query.clear();
        }
        self.open = true;
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut Settings,
        diagnostics: &mut Diagnostics,
        board: &super::Document,
        adapter: &str,
        enabled: &[bool],
    ) -> Vec<Intent> {
        if !self.open {
            return Vec::new();
        }
        let mut intents = Vec::new();
        let before = settings.preferences();
        let language = settings.language;
        let defaults = Settings::default();
        let mut open = self.open;
        let size = (ui.ctx().content_rect().size()
            - egui::Vec2::splat(moth_ui::tokens(ui.ctx()).space_xl as f32))
        .min(egui::vec2(760., 620.))
        .max(egui::Vec2::splat(1.));
        egui::Window::new(text(Text::Settings))
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .fixed_size(size)
            .movable(false)
            .resizable(false)
            .collapsible(false)
            .vscroll(false)
            .show(ui.ctx(), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .desired_width(f32::INFINITY)
                        .hint_text(text(Text::Search))
                        .char_limit(128),
                );
                let narrow = ui.available_width() < 550.;
                ui.with_layout(
                    if narrow {
                        egui::Layout::top_down(egui::Align::Min)
                    } else {
                        egui::Layout::left_to_right(egui::Align::Min)
                    },
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("settings_navigation")
                            .max_width(if narrow { ui.available_width() } else { 190. })
                            .max_height(if narrow {
                                ui.spacing().interact_size.y * 2.
                            } else {
                                ui.available_height()
                            })
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    if !narrow {
                                        ui.set_width(190.);
                                    }
                                    for section in Section::ALL {
                                        if moth_ui::icons::labeled_button(
                                            ui,
                                            section.icon(),
                                            text(section.label()),
                                            self.section.unwrap_or(Section::General) == section,
                                        )
                                        .clicked()
                                        {
                                            self.section = Some(section);
                                            self.query.clear();
                                        }
                                    }
                                });
                            });
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .id_salt("settings_content")
                            .max_height(ui.available_height())
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(ui.available_width());
                                    let section = self.section.unwrap_or(Section::General);
                                    let query = self.query.to_lowercase();
                                    for (index, entry) in
                                        SCHEMA.iter().enumerate().filter(|(_, e)| {
                                            if query.is_empty() {
                                                e.section == section
                                            } else {
                                                text(e.label).to_lowercase().contains(&query)
                                                    || e.key.contains(&query)
                                            }
                                        })
                                    {
                                        let value = (entry.get)(settings);
                                        let default = (entry.get)(&defaults);
                                        let label = format!(
                                            "{}{}",
                                            text(entry.label),
                                            if value != default { " *" } else { "" }
                                        );
                                        let row = ui.push_id(entry.key, |ui| {
                                            let mut next = value.clone();
                                            match entry.kind {
                                                Kind::VideoLimit => {
                                                    ui.label(label);
                                                    let mut auto = settings.vram_limit_mib == 0;
                                                    let max = diagnostics
                                                        .video
                                                        .as_ref()
                                                        .and_then(|v| v.dedicated.or(v.system))
                                                        .map_or(64 * 1024, |bytes| {
                                                            (bytes / (1024 * 1024)) as u32
                                                        })
                                                        .clamp(1024, MAX_VRAM_MIB);
                                                    if ui
                                                        .checkbox(
                                                            &mut auto,
                                                            text(Text::VideoLimitAuto),
                                                        )
                                                        .changed()
                                                    {
                                                        next = if auto {
                                                            "0".into()
                                                        } else {
                                                            diagnostics
                                                                .video
                                                                .as_ref()
                                                                .map_or(4096, |v| {
                                                                    (v.limit / (1024 * 1024)) as u32
                                                                })
                                                                .clamp(MIN_VRAM_MIB, max)
                                                                .to_string()
                                                        };
                                                    }
                                                    if !auto {
                                                        let mut limit = next
                                                            .parse::<u32>()
                                                            .unwrap_or(MIN_VRAM_MIB)
                                                            .clamp(MIN_VRAM_MIB, max);
                                                        if ui
                                                            .add(
                                                                egui::Slider::new(
                                                                    &mut limit,
                                                                    MIN_VRAM_MIB..=max,
                                                                )
                                                                .logarithmic(true)
                                                                .suffix(" MiB"),
                                                            )
                                                            .changed()
                                                        {
                                                            next = limit.to_string();
                                                        }
                                                    }
                                                    ui.small(text(Text::CacheHint));
                                                }

                                                Kind::Language => {
                                                    ui.label(label);
                                                    let mut language = settings.language;
                                                    egui::ComboBox::from_id_salt("language")
                                                        .selected_text(language.name())
                                                        .show_ui(ui, |ui| {
                                                            for candidate in moth_ui::Locale::ALL {
                                                                ui.selectable_value(
                                                                    &mut language,
                                                                    candidate,
                                                                    candidate.name(),
                                                                );
                                                            }
                                                        });
                                                    next = language.code().to_owned();
                                                }
                                                Kind::Color => {
                                                    ui.label(label);
                                                    let defaults = if settings.dark {
                                                        moth_ui::Tokens::DARK
                                                    } else {
                                                        moth_ui::Tokens::LIGHT
                                                    };
                                                    let fallback = match entry.key {
                                                        "background_color" => defaults.surface,
                                                        "selection_color" => defaults.selection,
                                                        _ => defaults.canvas,
                                                    }
                                                    .to_array();
                                                    let mut rgb =
                                                        moth_ui::theme::decode_color(&value)
                                                            .unwrap_or([
                                                                fallback[0],
                                                                fallback[1],
                                                                fallback[2],
                                                            ]);
                                                    if ui.color_edit_button_srgb(&mut rgb).changed()
                                                    {
                                                        next =
                                                            moth_ui::theme::encode_color(Some(rgb));
                                                    }
                                                    if ui
                                                        .small_button(text(Text::ResetColor))
                                                        .clicked()
                                                    {
                                                        next.clear();
                                                    }
                                                }
                                                Kind::Toggle => {
                                                    let mut v = value == "true";
                                                    if ui.checkbox(&mut v, label).changed() {
                                                        next = v.to_string();
                                                    }
                                                }
                                                Kind::Count(min, max) => {
                                                    let mut count =
                                                        value.parse::<u64>().unwrap_or(min);
                                                    ui.horizontal(|ui| {
                                                        ui.label(label);
                                                        if ui
                                                            .add(
                                                                egui::DragValue::new(&mut count)
                                                                    .range(min..=max),
                                                            )
                                                            .changed()
                                                        {
                                                            next = count.to_string();
                                                        }
                                                    });
                                                }
                                                Kind::Number(min, max, unit) => {
                                                    let mut v = value.parse::<f64>().unwrap_or(min);
                                                    ui.horizontal(|ui| {
                                                        ui.label(label);
                                                        if ui
                                                            .add(
                                                                egui::DragValue::new(&mut v)
                                                                    .range(min..=max)
                                                                    .suffix(unit),
                                                            )
                                                            .changed()
                                                        {
                                                            next = v.to_string();
                                                        }
                                                    });
                                                }
                                                Kind::Choice(options) => {
                                                    ui.label(label);
                                                    let codes: Vec<String> = match entry.key {
                                                        "performance_mode" => {
                                                            moth_io::CacheMode::ALL
                                                                .iter()
                                                                .map(|v| v.code().to_owned())
                                                                .collect()
                                                        }
                                                        "bake_mode" => moth_io::BakeMode::ALL
                                                            .iter()
                                                            .map(|v| v.code().to_owned())
                                                            .collect(),
                                                        _ => (0..options.len())
                                                            .map(|i| i.to_string())
                                                            .collect(),
                                                    };
                                                    let selected = codes
                                                        .iter()
                                                        .position(|code| code == &value)
                                                        .unwrap_or(0);
                                                    egui::ComboBox::from_id_salt("choice")
                                                        .selected_text(text(options[selected]))
                                                        .show_ui(ui, |ui| {
                                                            for (option, code) in
                                                                options.iter().zip(codes)
                                                            {
                                                                ui.selectable_value(
                                                                    &mut next,
                                                                    code,
                                                                    text(*option),
                                                                );
                                                            }
                                                        });
                                                }
                                            }
                                            if let Some(hint) = entry.hint {
                                                ui.small(text(hint));
                                            }
                                            if next != value {
                                                (entry.set)(
                                                    settings,
                                                    &BTreeMap::from([(entry.key.to_owned(), next)]),
                                                );
                                            }
                                        });
                                        if self.focus_row == Some(index) {
                                            row.response.scroll_to_me(Some(egui::Align::Center));
                                            self.focus_row = None;
                                        }
                                    }
                                    if self.query.is_empty() {
                                        if section == Section::Keys
                                            && ui.button(text(Text::Shortcuts)).clicked()
                                        {
                                            intents.push(Intent::Action(Action::BindingSettings));
                                        }
                                        if section == Section::Window {
                                            use moth_platform::{Platform, ShellIntegration};
                                            let integration =
                                                moth_platform::Desktop.shell_integration();
                                            match integration {
                                                ShellIntegration::Missing => {
                                                    ui.label(text(Text::ShellIntegrationMissing));
                                                }
                                                ShellIntegration::Pending => {
                                                    ui.label(text(Text::ShellIntegrationPending));
                                                }
                                                _ => {}
                                            }
                                            if integration == ShellIntegration::Missing
                                                && ui
                                                    .add_enabled(
                                                        enabled[Action::ShellIntegration as usize],
                                                        egui::Button::new(text(
                                                            Text::InstallShellIntegration,
                                                        )),
                                                    )
                                                    .clicked()
                                            {
                                                intents
                                                    .push(Intent::Action(Action::ShellIntegration));
                                            }
                                            for action in
                                                [Action::Overlay, Action::Pin, Action::Fullscreen]
                                            {
                                                if ui
                                                    .add_enabled(
                                                        enabled[action as usize],
                                                        egui::Button::new(text(action.label())),
                                                    )
                                                    .clicked()
                                                {
                                                    intents.push(Intent::Action(action));
                                                }
                                            }
                                        }
                                        if section == Section::Performance {
                                            if let Some(usage) = &diagnostics.disk_cache_usage {
                                                ui.label(format!(
                                                    "{}: {}",
                                                    text(Text::DiskCacheUsage),
                                                    match usage {
                                                        Ok(bytes) => format!(
                                                            "{:.1} MiB",
                                                            *bytes as f64 / 1048576.
                                                        ),
                                                        Err(e) => e.clone(),
                                                    }
                                                ));
                                            }
                                            if ui
                                                .add_enabled(
                                                    !diagnostics.disk_cache_busy,
                                                    egui::Button::new(text(Text::ClearDiskCache)),
                                                )
                                                .clicked()
                                            {
                                                intents
                                                    .push(Intent::Action(Action::ClearDiskCache));
                                            }
                                        }
                                        if section == Section::Developers {
                                            ui.label(adapter);
                                            let cache = board.decode().cache();
                                            ui.label(format!(
                                                "{}: {} / {}",
                                                text(Text::RamCacheUsage),
                                                cache.memory_bytes(),
                                                cache.memory_budget()
                                            ));
                                            ui.label(format!(
                                                "{}: {}",
                                                text(Text::Journaled),
                                                board.intake.in_flight
                                            ));
                                            if let Some(video) = &diagnostics.video {
                                                ui.label(super::tools::video_summary(video));
                                            }
                                        }
                                        if ui.button(text(Text::ResetSection)).clicked() {
                                            for entry in
                                                SCHEMA.iter().filter(|e| e.section == section)
                                            {
                                                (entry.set)(
                                                    settings,
                                                    &BTreeMap::from([(
                                                        entry.key.to_owned(),
                                                        (entry.get)(&defaults),
                                                    )]),
                                                );
                                            }
                                        }
                                    }
                                });
                            });
                    },
                );
            });
        self.open = open;
        if language != settings.language {
            intents.push(Intent::Language(settings.language));
        }
        if before != settings.preferences() {
            intents.push(Intent::PreferencesDirty);
        }
        intents
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn minimap_opacity_and_distance_persist_with_validated_percentages() {
        for value in [0., 35., 100.] {
            let mut settings = Settings::default();
            settings.load(&BTreeMap::from([
                ("minimap_opacity".into(), value.to_string()),
                ("minimap_distance".into(), value.to_string()),
            ]));
            let mut restored = Settings::default();
            restored.load(
                &settings
                    .preferences()
                    .into_iter()
                    .map(|(key, value)| (key.to_owned(), value))
                    .collect(),
            );
            assert_eq!(restored.minimap_opacity, value);
            assert_eq!(restored.minimap_distance, value);
        }
        for invalid in ["NaN", "inf", "-1", "101"] {
            let mut settings = Settings::default();
            settings.load(&BTreeMap::from([
                ("minimap_opacity".into(), invalid.into()),
                ("minimap_distance".into(), invalid.into()),
            ]));
            assert_eq!(settings.minimap_opacity, 100.);
            assert_eq!(settings.minimap_distance, 100.);
        }
    }
    #[test]
    fn minimap_positions_persist_and_cover_all_nine_anchors() {
        let mut anchors = Vec::new();
        for position in 0..9 {
            let mut settings = Settings::default();
            settings.load(&BTreeMap::from([(
                "minimap_position".into(),
                position.to_string(),
            )]));
            let mut restored = Settings::default();
            restored.load(
                &settings
                    .preferences()
                    .into_iter()
                    .map(|(key, value)| (key.to_owned(), value))
                    .collect(),
            );
            assert_eq!(restored.minimap_position, position);
            let anchor = restored.minimap_alignment();
            assert!(!anchors.contains(&anchor));
            anchors.push(anchor);
        }
        let mut settings = Settings::default();
        settings.load(&BTreeMap::from([("minimap_position".into(), "9".into())]));
        assert_eq!(settings.minimap_alignment(), egui::Align2::LEFT_BOTTOM);
    }
    #[test]
    fn minimap_preferences_persist_and_reject_invalid_sizes() {
        let mut settings = Settings::default();
        settings.load(&BTreeMap::from([
            ("show_minimap".into(), "false".into()),
            ("minimap_size".into(), "320".into()),
        ]));
        let mut restored = Settings::default();
        restored.load(
            &settings
                .preferences()
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        );
        assert!(!restored.show_minimap);
        assert_eq!(restored.minimap_size, 320.);
        for invalid in ["NaN", "inf", "-1", "0", "601"] {
            restored.load(&BTreeMap::from([("minimap_size".into(), invalid.into())]));
            assert_eq!(restored.minimap_size, moth_ui::Tokens::LIGHT.minimap_max);
        }
    }
    #[test]
    fn keyboard_pan_preferences_roundtrip_and_reject_invalid_values() {
        let mut settings = Settings::default();
        settings.load(&BTreeMap::from([
            ("keyboard_pan_mode".into(), "0".into()),
            ("keyboard_pan_step".into(), "400".into()),
            ("keyboard_pan_speed".into(), "1200".into()),
        ]));
        let mut restored = Settings::default();
        restored.load(
            &settings
                .preferences()
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        );
        assert_eq!(restored.keyboard_pan_mode, 0);
        assert_eq!(restored.keyboard_pan_step, 400.);
        assert_eq!(restored.keyboard_pan_speed, 1200.);
        for invalid in ["NaN", "inf", "-1", "0", "999999"] {
            restored.load(&BTreeMap::from([
                ("keyboard_pan_mode".into(), "2".into()),
                ("keyboard_pan_step".into(), invalid.into()),
                ("keyboard_pan_speed".into(), invalid.into()),
            ]));
            assert_eq!(restored.keyboard_pan_mode, 1);
            assert_eq!(restored.keyboard_pan_step, 200.);
            assert_eq!(restored.keyboard_pan_speed, 800.);
        }
    }
    #[test]
    fn sampling_preference_roundtrips_and_rejects_unknown_modes() {
        for code in 0..3 {
            let mut settings = Settings::default();
            settings.load(&BTreeMap::from([("sampling".into(), code.to_string())]));
            assert_eq!(settings.sampling, code);
            let mut restored = Settings::default();
            restored.load(
                &settings
                    .preferences()
                    .into_iter()
                    .map(|(key, value)| (key.to_owned(), value))
                    .collect(),
            );
            assert_eq!(restored.sampling, code);
        }
        let mut settings = Settings::default();
        settings.load(&BTreeMap::from([("sampling".into(), "255".into())]));
        assert_eq!(settings.sampling, 0);
    }
    #[test]
    fn paper_default_preserves_an_explicit_theme_preference() {
        let mut settings = Settings::default();
        assert!(!settings.dark);
        settings.load(&BTreeMap::new());
        assert!(!settings.dark);
        assert_eq!(settings.threshold, 5);
        assert!(!settings.auto_arrange);
        settings.load(&BTreeMap::from([("dark".into(), "true".into())]));
        assert!(settings.dark);
    }

    #[test]
    fn adding_preferences_hide_packing_details_in_developers() {
        for key in ["arrange_choice", "arrange_split"] {
            let entry = SCHEMA.iter().find(|entry| entry.key == key).unwrap();
            assert!(entry.section == Section::Developers);
        }
        for (key, label) in [
            ("arrange_gap", Text::ImageGap),
            ("default_arrange", Text::NewImageLayout),
            ("arrange_prompt_threshold", Text::ManyImages),
        ] {
            let entry = SCHEMA.iter().find(|entry| entry.key == key).unwrap();
            assert!(entry.section == Section::Adding);
            assert_eq!(entry.label, label);
        }
    }

    #[test]
    fn all_settings_and_bindings_persist_together() {
        let root =
            std::env::temp_dir().join(format!("moth-preferences-{}", moth_scene::Uuid::now_v7()));
        let mut values: BTreeMap<_, _> = Settings::default()
            .preferences()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect();
        values.extend(super::super::bindings::Registry::default().preferences());
        moth_platform::settings::save_all(&root, &values).unwrap();
        assert_eq!(moth_platform::settings::load(&root).unwrap(), values);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn schema_roundtrips_and_rejects_invalid_values() {
        let mut settings = Settings::default();
        let original = settings.preferences();
        assert!(
            original.len() + super::super::bindings::ACTIONS.len()
                <= moth_platform::settings::MAX_ENTRIES,
            "preferences must fit the existing storage request budget"
        );
        settings.load(
            &original
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        );
        assert_eq!(settings.preferences(), original);
        settings.load(&BTreeMap::from([
            ("drag_opacity".into(), "NaN".into()),
            ("arrange_gap".into(), "0".into()),
            ("vram_limit_mib".into(), "255".into()),
        ]));
        assert_eq!(
            settings.drag_opacity,
            super::super::motion::DEFAULT_DRAG_OPACITY
        );
        assert_eq!(settings.gap, 24.);
        assert_eq!(settings.vram_limit_mib, 0);
        let mut keys = std::collections::HashSet::new();
        for entry in SCHEMA {
            assert!(keys.insert(entry.key));
        }
    }
}

#[cfg(test)]
mod window_preferences_tests {
    use super::*;
    #[test]
    fn custom_colors_and_native_shortcut_preferences_round_trip() {
        let mut settings = Settings {
            background_color: Some([1, 2, 3]),
            canvas_color: Some([4, 5, 6]),
            selection_color: Some([7, 8, 9]),
            view_controls_right: false,
            global_shortcuts: true,
            lock_maximized: false,
            disable_click_through_on_activate: false,
            ..Default::default()
        };
        let values = settings
            .preferences()
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        let mut restored = Settings::default();
        restored.load(&values);
        assert_eq!(settings.tokens(), restored.tokens());
        assert!(!restored.view_controls_right);
        assert!(restored.global_shortcuts);
        assert!(!restored.lock_maximized);
        assert!(!restored.disable_click_through_on_activate);
        settings.load(&BTreeMap::from([
            ("canvas_color".into(), "oops".into()),
            ("global_shortcuts".into(), "oops".into()),
            ("view_controls_right".into(), "oops".into()),
        ]));
        assert_eq!(settings.canvas_color, None);
        assert!(settings.view_controls_right);
        assert!(!settings.global_shortcuts);
        assert!(settings.lock_maximized);
        assert!(settings.disable_click_through_on_activate);
    }
}
