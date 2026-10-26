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
    language: moth_ui::Locale = moth_ui::locale(), "language", General, Language, Kind::Language, |s| s.language.code().to_owned(), |values: &BTreeMap<String,String>| values.get("language").and_then(|v|moth_ui::Locale::from_code(v)).unwrap_or_else(moth_ui::locale);

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
    rotation_snap: bool = false, "rotation_snap", Canvas, SnapRotation, Kind::Toggle, |s| s.rotation_snap.to_string(), |values: &BTreeMap<String,String>| values
            .get("rotation_snap")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
    scale_with_window: bool = false, "scale_with_window", Canvas, ScaleWithWindow, Kind::Toggle, |s| s.scale_with_window.to_string(), |values: &BTreeMap<String,String>| values
            .get("scale_with_window")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
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
    pub fn preferences(&self) -> Vec<(&'static str, String)> {
        SCHEMA
            .iter()
            .map(|entry| (entry.key, (entry.get)(self)))
            .collect()
    }
    pub fn load(&mut self, values: &BTreeMap<String, String>) {
        for entry in SCHEMA {
            (entry.set)(self, values);
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
        tools: &mut super::tools::Tools,
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
                                                    let max = tools
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
                                                            tools
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
                                            if let Some(usage) = &tools.disk_cache_usage {
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
                                                    !tools.disk_cache_busy,
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
                                                board.in_flight
                                            ));
                                            if let Some(video) = &tools.video {
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
