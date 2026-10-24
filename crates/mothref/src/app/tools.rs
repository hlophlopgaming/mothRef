// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit Apply keeps inspector drafts out of the journal until confirmed.
use super::{Document, arrange, intent::Intent, snap};
use moth_platform::export::{Geometry, Target};
use moth_scene::{Command, Rect, Transform, Uuid};
use moth_ui::{Text, text};

pub(super) struct Tools {
    draft: Option<(Uuid, Rect, Transform)>,
    observed: Option<(Uuid, Rect, Transform)>,
    pub gap: f32,
    pub layout_counts: [usize; 2],
    pub shortcut_feedback: bool,
    pub(super) order: u8,
    last_arrange: Option<(u8, u8, u64)>,
    reversed: bool,
    random_salt: u128,
    pub locked: bool,
    pub grayscale: bool,
    pub dark: bool,
    pub reduced_motion: bool,
    pub show_outlines: bool,
    pub drag_animation: bool,
    pub drag_opacity: f32,
    pub ui_scale: f32,
    pub threshold: usize,
    pub undo_limit: usize,
    /// Texture limit in MiB set by the user; 0 follows the detected video memory.
    pub vram_limit_mib: u32,
    /// Detected adapter memory, copied in by the app each frame (not persisted).
    pub video: Option<moth_platform::gpu::VideoMemory>,
    pub disk_cache_usage: Option<Result<u64, String>>,
    pub disk_cache_busy: bool,
    pub clear_disk_cache: bool,
    /// How decoded previews are kept between zoom steps.
    pub performance_mode: moth_io::CacheMode,
    /// Which large images get their zoom tiles prepared in the background.
    pub bake_mode: moth_io::BakeMode,
    /// Images of the open board waiting for tile preparation (not persisted).
    pub bake_pending: usize,
    pub wheel: u8,
    pub smooth_zoom: bool,
    pub zoom_increment: f32,
    pub rotation_snap: bool,
    pub scale_with_window: bool,
    pub auto_arrange: bool,
    pub default_arrange: u8,
    pub arrange_choice: u8,
    pub arrange_split: u8,
    pub coordinates: bool,
    factor: f32,
    path: String,
    side: u32,
    region: Rect,
    crop: bool,
    pub picking: bool,
    pub color_format: u8,
    pub open: bool,
}
impl Default for Tools {
    fn default() -> Self {
        Self {
            draft: None,
            observed: None,
            gap: 24.,
            layout_counts: [1, 1],
            shortcut_feedback: true,
            order: 0,
            last_arrange: None,
            reversed: false,
            random_salt: 0,
            locked: false,
            grayscale: false,
            dark: true,
            reduced_motion: false,
            show_outlines: true,
            drag_animation: true,
            drag_opacity: super::motion::DEFAULT_DRAG_OPACITY,
            ui_scale: 1.,
            threshold: 5,
            undo_limit: 256,
            vram_limit_mib: 0,
            video: None,
            disk_cache_usage: None,
            disk_cache_busy: false,
            clear_disk_cache: false,
            performance_mode: moth_io::CacheMode::default(),
            bake_mode: moth_io::BakeMode::default(),
            bake_pending: 0,
            wheel: 0,
            smooth_zoom: true,
            zoom_increment: 10.,
            rotation_snap: false,
            scale_with_window: false,
            auto_arrange: false,
            default_arrange: 0,
            arrange_choice: 1,
            arrange_split: 0,
            coordinates: false,
            factor: 1.,
            path: String::new(),
            side: 2048,
            region: Rect {
                x: 0.,
                y: 0.,
                width: 1024.,
                height: 1024.,
            },
            crop: false,
            picking: false,
            color_format: 0,
            open: false,
        }
    }
}
/// Label and explanation of a performance mode.
fn mode_text(mode: moth_io::CacheMode) -> (Text, Text) {
    match mode {
        moth_io::CacheMode::Memory => (Text::ModeMemory, Text::ModeMemoryHint),
        moth_io::CacheMode::MemoryLowQuality => (Text::ModeMemoryLow, Text::ModeMemoryLowHint),
        moth_io::CacheMode::Disk => (Text::ModeDisk, Text::ModeDiskHint),
        moth_io::CacheMode::NoCache => (Text::ModeNoCache, Text::ModeNoCacheHint),
        moth_io::CacheMode::Full => (Text::ModeFull, Text::ModeFullHint),
    }
}
/// Smallest manual texture limit.
const MIN_VRAM_MIB: u32 = 256;
/// Largest manual texture limit accepted from preferences (1 TiB).
const MAX_VRAM_MIB: u32 = 1 << 20;
const MIB: u64 = 1024 * 1024;
fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / (1024. * MIB as f64))
}
/// Adapter, its memory and the limit derived from it, for settings and the status line.
pub(super) fn video_summary(video: &moth_platform::gpu::VideoMemory) -> String {
    let memory = match video.dedicated {
        Some(bytes) => gib(bytes),
        None => format!(
            "{}{}",
            text(Text::VideoShared),
            video
                .system
                .map(|b| format!(" ({})", gib(b)))
                .unwrap_or_default()
        ),
    };
    format!(
        "{}: {} · {} — {}: {}",
        text(Text::VideoCard),
        video.name,
        memory,
        text(Text::VideoLimitIs),
        gib(video.limit)
    )
}
fn bake_text(mode: moth_io::BakeMode) -> Text {
    match mode {
        moth_io::BakeMode::Visible => Text::BakeVisible,
        moth_io::BakeMode::Board => Text::BakeBoard,
        moth_io::BakeMode::Off => Text::BakeOff,
    }
}
fn preference(key: Text, changed: bool) -> String {
    format!("{}{}", text(key), if changed { " *" } else { "" })
}
fn number(ui: &mut egui::Ui, key: Text, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.horizontal(|ui| {
        ui.label(text(key));
        ui.add(egui::DragValue::new(value).range(range));
    });
}
fn rect(ui: &mut egui::Ui, rect: &mut Rect) {
    number(ui, Text::PositionX, &mut rect.x, -1e9..=1e9);
    number(ui, Text::PositionY, &mut rect.y, -1e9..=1e9);
    number(ui, Text::Width, &mut rect.width, 0.001..=1e9);
    number(ui, Text::Height, &mut rect.height, 0.001..=1e9);
}
impl Tools {
    pub fn arrange(
        &mut self,
        items: &mut Vec<&moth_scene::Item>,
        mode: arrange::Arrange,
    ) -> Vec<Command> {
        use std::hash::{Hash, Hasher};
        let mut ids: Vec<_> = items.iter().map(|i| i.id).collect();
        ids.sort_unstable();
        let mut hash = std::hash::DefaultHasher::new();
        ids.hash(&mut hash);
        self.layout_counts.hash(&mut hash);
        let key = (self.order, mode as u8, hash.finish());
        let repeat = self.last_arrange == Some(key);
        self.reversed = repeat && !self.reversed;
        self.last_arrange = Some(key);
        if !repeat {
            self.random_salt = Uuid::now_v7().as_u128();
        }
        match self.order {
            1 => items.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id))),
            2 => items.sort_by_key(|i| i.id),
            3 => items.sort_by_key(|i| {
                i.id.as_u128()
                    .wrapping_mul(self.random_salt | 1)
                    .rotate_left(43)
            }),
            _ => items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id))),
        }
        if self.reversed {
            items.reverse();
        }
        if matches!(
            mode,
            arrange::Arrange::Horizontal | arrange::Arrange::Vertical
        ) {
            return arrange::counted(items, mode, self.layout_counts[mode as usize], self.gap);
        }
        arrange::arrange(
            items,
            mode,
            self.gap,
            arrange::FreeRectChoice::from_index(self.arrange_choice),
            arrange::GuillotineSplit::from_index(self.arrange_split),
        )
    }
    /// Texture limit in bytes: the user's value, else the detected one (`None` until
    /// detection finishes).
    pub fn vram_limit(&self) -> Option<u64> {
        if self.vram_limit_mib > 0 {
            Some(u64::from(self.vram_limit_mib) * MIB)
        } else {
            self.video.as_ref().map(|video| video.limit)
        }
    }
    pub fn preferences(&self) -> Vec<(&'static str, String)> {
        vec![
            ("shortcut_feedback", self.shortcut_feedback.to_string()),
            (
                moth_platform::settings::REDUCED_MOTION,
                self.reduced_motion.to_string(),
            ),
            ("show_outlines", self.show_outlines.to_string()),
            ("drag_animation", self.drag_animation.to_string()),
            ("drag_opacity", self.drag_opacity.to_string()),
            ("wheel", self.wheel.to_string()),
            ("smooth_zoom", self.smooth_zoom.to_string()),
            ("zoom_increment", self.zoom_increment.to_string()),
            ("rotation_snap", self.rotation_snap.to_string()),
            ("scale_with_window", self.scale_with_window.to_string()),
            ("undo_limit", self.undo_limit.to_string()),
            ("vram_limit_mib", self.vram_limit_mib.to_string()),
            ("performance_mode", self.performance_mode.code().to_owned()),
            ("bake_mode", self.bake_mode.code().to_owned()),
            ("dark", self.dark.to_string()),
            ("ui_scale", self.ui_scale.to_string()),
            ("arrange_gap", self.gap.to_string()),
            ("arrange_prompt_threshold", self.threshold.to_string()),
            ("auto_arrange", self.auto_arrange.to_string()),
            ("default_arrange", self.default_arrange.to_string()),
            ("arrange_choice", self.arrange_choice.to_string()),
            ("arrange_split", self.arrange_split.to_string()),
        ]
    }
    pub fn load(&mut self, values: &std::collections::BTreeMap<String, String>) {
        self.shortcut_feedback = values
            .get("shortcut_feedback")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
        // The old 32–512 MiB `cache_limit_mib` is ignored: limits now follow the adapter.
        self.vram_limit_mib = values
            .get("vram_limit_mib")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v == 0 || (MIN_VRAM_MIB..=MAX_VRAM_MIB).contains(v))
            .unwrap_or(0);
        self.performance_mode = values
            .get("performance_mode")
            .and_then(|v| moth_io::CacheMode::from_code(v.as_str()))
            .unwrap_or_default();
        self.bake_mode = values
            .get("bake_mode")
            .and_then(|v| moth_io::BakeMode::from_code(v.as_str()))
            .unwrap_or_default();
        self.reduced_motion = values
            .get(moth_platform::settings::REDUCED_MOTION)
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
        self.show_outlines = values
            .get("show_outlines")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
        self.drag_animation = values
            .get("drag_animation")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
        self.drag_opacity = values
            .get("drag_opacity")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && (0.1..=1.).contains(v))
            .unwrap_or(super::motion::DEFAULT_DRAG_OPACITY);
        self.wheel = values
            .get("wheel")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v < 3)
            .unwrap_or(0);
        self.smooth_zoom = values
            .get("smooth_zoom")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
        self.zoom_increment = values
            .get("zoom_increment")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && (1.0..=100.0).contains(v))
            .unwrap_or(10.);
        self.rotation_snap = values
            .get("rotation_snap")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
        self.scale_with_window = values
            .get("scale_with_window")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
        self.undo_limit = values
            .get("undo_limit")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v <= 4096)
            .unwrap_or(256);
        self.dark = values
            .get("dark")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
        self.ui_scale = values
            .get("ui_scale")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && (0.75..=2.).contains(v))
            .unwrap_or(1.);
        self.gap = values
            .get("arrange_gap")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && (1.0..=10000.).contains(v))
            .unwrap_or(24.);
        self.threshold = values
            .get("arrange_prompt_threshold")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v <= 10000)
            .unwrap_or(5);
        self.auto_arrange = values
            .get("auto_arrange")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
        self.default_arrange = values
            .get("default_arrange")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v < 4)
            .unwrap_or(0);
        self.arrange_choice = values
            .get("arrange_choice")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v < 6)
            .unwrap_or(1);
        self.arrange_split = values
            .get("arrange_split")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v < 6)
            .unwrap_or(0);
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        board: &Document,
        editing: bool,
    ) -> (Vec<Command>, Option<Target>, bool, Vec<Intent>) {
        let mut commands = Vec::new();
        let mut export = None;
        let mut note = false;
        let mut intents = Vec::new();
        let mut open = self.open;
        egui::Window::new(text(Text::Tools))
            .open(&mut open)
            .fade_in(!self.reduced_motion)
            .fade_out(!self.reduced_motion)
            .default_width(280.)
            .max_width(ui.content_rect().width().max(1.))
            .max_height(ui.content_rect().height().max(1.))
            .vscroll(true)
            .show(ui.ctx(), |ui| {
                ui.spacing_mut().interact_size.y = ui.spacing().interact_size.y.max(36.);
                ui.collapsing(text(Text::Settings), |ui| {
                    let before = self.preferences();
                    let defaults = Self::default();
                    ui.collapsing(text(Text::ImageCache), |ui| {
                        let (selected, hint) = mode_text(self.performance_mode);
                        ui.label(preference(
                            Text::PerformanceMode,
                            self.performance_mode != defaults.performance_mode,
                        ));
                        egui::ComboBox::from_id_salt("performance_mode")
                            .selected_text(text(selected))
                            .show_ui(ui, |ui| {
                                for mode in moth_io::CacheMode::ALL {
                                    ui.selectable_value(
                                        &mut self.performance_mode,
                                        mode,
                                        text(mode_text(mode).0),
                                    );
                                }
                            });
                        ui.label(text(hint));
                        ui.separator();
                        ui.label(preference(
                            Text::BakeTiles,
                            self.bake_mode != defaults.bake_mode,
                        ));
                        egui::ComboBox::from_id_salt("bake_mode")
                            .selected_text(text(bake_text(self.bake_mode)))
                            .show_ui(ui, |ui| {
                                for mode in moth_io::BakeMode::ALL {
                                    ui.selectable_value(
                                        &mut self.bake_mode,
                                        mode,
                                        text(bake_text(mode)),
                                    );
                                }
                            });
                        ui.label(text(Text::BakeHint));
                        if self.bake_pending > 0 {
                            ui.label(format!(
                                "{}: {}",
                                text(Text::BakePending),
                                self.bake_pending
                            ));
                        }
                        ui.separator();
                        ui.label(preference(
                            Text::CacheLimit,
                            self.vram_limit_mib != defaults.vram_limit_mib,
                        ));
                        let detected_mib = self
                            .video
                            .as_ref()
                            .map(|v| (v.limit / MIB).clamp(1, u64::from(MAX_VRAM_MIB)) as u32);
                        let max_mib = self
                            .video
                            .as_ref()
                            .and_then(|v| v.dedicated.or(v.system))
                            .map_or(64 * 1024, |bytes| (bytes / MIB) as u32)
                            .clamp(1024, MAX_VRAM_MIB);
                        let mut auto = self.vram_limit_mib == 0;
                        if ui.checkbox(&mut auto, text(Text::VideoLimitAuto)).changed() {
                            self.vram_limit_mib = if auto {
                                0
                            } else {
                                detected_mib.unwrap_or(4096).clamp(MIN_VRAM_MIB, max_mib)
                            };
                        }
                        if !auto {
                            ui.add(
                                egui::Slider::new(&mut self.vram_limit_mib, MIN_VRAM_MIB..=max_mib)
                                    .logarithmic(true)
                                    .suffix(" MiB"),
                            );
                        }
                        match &self.video {
                            Some(video) => {
                                ui.label(video_summary(video));
                            }
                            None => {
                                ui.label(text(Text::VideoDetecting));
                            }
                        }
                        ui.label(text(Text::CacheHint));
                        let cache = board.decode().cache();
                        ui.label(format!(
                            "{}: {:.1} / {:.1} MiB",
                            text(Text::RamCacheUsage),
                            cache.memory_bytes() as f64 / 1048576.,
                            cache.memory_budget() as f64 / 1048576.
                        ));
                        match &self.disk_cache_usage {
                            Some(Ok(bytes)) => {
                                ui.label(format!(
                                    "{}: {:.1} MiB",
                                    text(Text::DiskCacheUsage),
                                    *bytes as f64 / 1048576.
                                ));
                            }
                            Some(Err(error)) => {
                                ui.label(format!("{}: {error}", text(Text::DiskCacheUsage)));
                            }
                            None => {
                                ui.label(text(Text::CachePending));
                            }
                        }
                        if ui
                            .add_enabled(
                                !self.disk_cache_busy && !self.clear_disk_cache,
                                egui::Button::new(text(Text::ClearDiskCache)),
                            )
                            .clicked()
                        {
                            self.clear_disk_cache = true;
                        }
                    });
                    ui.checkbox(&mut self.shortcut_feedback, text(Text::ShortcutFeedback));
                    ui.checkbox(&mut self.reduced_motion, text(Text::ReducedMotion));
                    ui.checkbox(&mut self.show_outlines, text(Text::ShowOutlines));
                    ui.checkbox(&mut self.drag_animation, text(Text::DragAnimation));
                    ui.add_enabled(
                        !self.drag_animation || self.reduced_motion,
                        egui::Slider::new(&mut self.drag_opacity, 0.1..=1.)
                            .text(text(Text::DragOpacity)),
                    );

                    let label = preference(
                        Text::ScaleWithWindow,
                        self.scale_with_window != defaults.scale_with_window,
                    );
                    ui.checkbox(&mut self.scale_with_window, label);
                    egui::ComboBox::from_id_salt("wheel")
                        .selected_text(preference(
                            [Text::WheelZoom, Text::WheelPan, Text::WheelOff][self.wheel as usize],
                            self.wheel != defaults.wheel,
                        ))
                        .show_ui(ui, |ui| {
                            for (i, key) in [Text::WheelZoom, Text::WheelPan, Text::WheelOff]
                                .into_iter()
                                .enumerate()
                            {
                                ui.selectable_value(&mut self.wheel, i as u8, text(key));
                            }
                        });
                    ui.checkbox(&mut self.smooth_zoom, text(Text::SmoothZoom));
                    ui.add(
                        egui::Slider::new(&mut self.zoom_increment, 1.0..=100.0)
                            .text(text(Text::ZoomIncrement)),
                    );
                    ui.checkbox(&mut self.rotation_snap, text(Text::SnapRotation));
                    let label = preference(Text::UndoLimit, self.undo_limit != defaults.undo_limit);
                    ui.add(egui::Slider::new(&mut self.undo_limit, 0..=4096).text(label));
                    let bytes = board
                        .scene
                        .items()
                        .take(128)
                        .map(|i| {
                            std::mem::size_of::<moth_scene::Command>()
                                + i.name.len()
                                + i.note().map_or(0, str::len)
                        })
                        .sum::<usize>()
                        / board.scene.len().clamp(1, 128);
                    ui.label(format!(
                        "≈ {} KiB",
                        bytes.saturating_mul(self.undo_limit) / 1024
                    ));
                    let label = preference(Text::DarkTheme, self.dark != defaults.dark);
                    ui.checkbox(&mut self.dark, label);
                    let label = preference(Text::UiScale, self.ui_scale != defaults.ui_scale);
                    ui.add(egui::Slider::new(&mut self.ui_scale, 0.75..=2.).text(label));
                    ui.horizontal(|ui| {
                        ui.label(preference(Text::Gap, self.gap != defaults.gap));
                        ui.add(egui::DragValue::new(&mut self.gap).range(1.0..=10000.));
                    });
                    ui.horizontal(|ui| {
                        ui.label(preference(
                            Text::ArrangeThreshold,
                            self.threshold != defaults.threshold,
                        ));
                        ui.add(egui::DragValue::new(&mut self.threshold).range(0..=10000));
                    });
                    let label = preference(
                        Text::AutoArrange,
                        self.auto_arrange != defaults.auto_arrange,
                    );
                    ui.checkbox(&mut self.auto_arrange, label);
                    egui::ComboBox::from_id_salt("default_arrange")
                        .selected_text(preference(
                            layout(self.default_arrange).0,
                            self.default_arrange != defaults.default_arrange,
                        ))
                        .show_ui(ui, |ui| {
                            for i in 0..4 {
                                let (label, mode) = layout(i);
                                if moth_ui::icons::labeled_button(
                                    ui,
                                    layout_icon(mode),
                                    text(label),
                                    self.default_arrange == i,
                                )
                                .clicked()
                                {
                                    self.default_arrange = i;
                                    ui.close();
                                }
                            }
                        });
                    egui::ComboBox::from_id_salt("arrange_choice")
                        .selected_text(preference(
                            choice_label(self.arrange_choice),
                            self.arrange_choice != defaults.arrange_choice,
                        ))
                        .show_ui(ui, |ui| {
                            for i in 0..6 {
                                ui.selectable_value(
                                    &mut self.arrange_choice,
                                    i,
                                    text(choice_label(i)),
                                );
                            }
                        });
                    egui::ComboBox::from_id_salt("arrange_split")
                        .selected_text(preference(
                            split_label(self.arrange_split),
                            self.arrange_split != defaults.arrange_split,
                        ))
                        .show_ui(ui, |ui| {
                            for i in 0..6 {
                                ui.selectable_value(
                                    &mut self.arrange_split,
                                    i,
                                    text(split_label(i)),
                                );
                            }
                        });
                    if before != self.preferences() {
                        intents.push(Intent::PreferencesDirty);
                    }
                    if self.preferences() != defaults.preferences() {
                        ui.label(text(Text::ChangedSetting));
                    }
                    if self.preferences() != defaults.preferences()
                        && ui
                            .button(moth_ui::tooltip(ui.ctx(), Text::Defaults))
                            .clicked()
                    {
                        self.shortcut_feedback = defaults.shortcut_feedback;
                        self.wheel = defaults.wheel;
                        self.smooth_zoom = defaults.smooth_zoom;
                        self.zoom_increment = defaults.zoom_increment;
                        self.rotation_snap = defaults.rotation_snap;
                        self.scale_with_window = defaults.scale_with_window;
                        self.undo_limit = defaults.undo_limit;
                        self.vram_limit_mib = defaults.vram_limit_mib;
                        self.performance_mode = defaults.performance_mode;
                        self.bake_mode = defaults.bake_mode;
                        self.dark = defaults.dark;
                        self.ui_scale = defaults.ui_scale;
                        self.gap = defaults.gap;
                        self.threshold = defaults.threshold;
                        self.auto_arrange = defaults.auto_arrange;
                        self.default_arrange = defaults.default_arrange;
                        self.arrange_choice = defaults.arrange_choice;
                        self.arrange_split = defaults.arrange_split;
                        intents.push(Intent::PreferencesDirty);
                    }
                });
                ui.add_enabled(
                    !editing && board.idle(),
                    egui::Checkbox::new(&mut self.locked, text(Text::LockCanvas)),
                );
                ui.checkbox(&mut self.grayscale, text(Text::CanvasGrayscale));
                intents.push(Intent::SetViewFlags {
                    locked: self.locked,
                    grayscale: self.grayscale,
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button(moth_ui::tooltip(ui.ctx(), Text::Fit)).clicked() {
                        intents.push(Intent::NeedsFit);
                    }
                    if ui
                        .button(moth_ui::tooltip(ui.ctx(), Text::ResetCamera))
                        .clicked()
                    {
                        intents.push(Intent::ResetCamera);
                    }
                    if ui
                        .button(moth_ui::tooltip(ui.ctx(), Text::PreviousImage))
                        .clicked()
                    {
                        intents.push(Intent::Focus(-1));
                    }
                    if ui
                        .button(moth_ui::tooltip(ui.ctx(), Text::NextImage))
                        .clicked()
                    {
                        intents.push(Intent::Focus(1));
                    }
                });
                ui.add_enabled_ui(board.idle(), |ui| {
                    let mut items: Vec<_> = board
                        .scene
                        .items()
                        .filter(|item| {
                            board.selection.is_empty() || board.selection.contains(&item.id)
                        })
                        .collect();
                    items.sort_unstable_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                    ui.add_enabled_ui(!self.locked, |ui| {
                        note = ui
                            .button(moth_ui::tooltip(ui.ctx(), Text::AddNote))
                            .clicked();
                        ui.label(format!(
                            "{}: {}",
                            text(Text::EditSelection),
                            board.selection.len()
                        ));
                        ui.add_enabled_ui(!board.selection.is_empty(), |ui| {
                            for (label, quarters) in [(Text::RotateCw, 1.), (Text::RotateCcw, -1.)]
                            {
                                if ui.button(moth_ui::tooltip(ui.ctx(), label)).clicked() {
                                    commands.extend(super::transform_selection(
                                        &board.scene,
                                        &board.selection,
                                        |t| super::rotated(t, quarters),
                                    ));
                                }
                            }
                            for (label, axis) in
                                [(Text::FlipHorizontal, 0), (Text::FlipVertical, 1)]
                            {
                                if ui.button(moth_ui::tooltip(ui.ctx(), label)).clicked() {
                                    commands.extend(super::transform_selection(
                                        &board.scene,
                                        &board.selection,
                                        |t| super::mirrored(t, axis),
                                    ));
                                }
                            }
                            if ui
                                .button(moth_ui::tooltip(ui.ctx(), Text::Delete))
                                .clicked()
                            {
                                commands
                                    .extend(board.selection.iter().copied().map(Command::Remove));
                            }
                        });
                        if items.len() == 1 {
                            let item = items[0];
                            if self.observed != Some((item.id, item.rect, item.transform)) {
                                self.observed = Some((item.id, item.rect, item.transform));
                                self.draft = Some((item.id, item.rect, item.transform));
                            }
                            let rotation_snap = self.rotation_snap;
                            ui.collapsing(text(Text::EditSelection), |ui| {
                                let Some((id, frame, t)) = &mut self.draft else {
                                    return;
                                };
                                ui.label(&item.name);
                                rect(ui, frame);
                                let mut degrees = t.rotation.to_degrees();
                                number(ui, Text::Rotation, &mut degrees, -36000.0..=36000.0);
                                t.rotation = degrees.to_radians();
                                if rotation_snap {
                                    t.rotation =
                                        snap::snap_angle(t.rotation, 15.0_f32.to_radians());
                                }
                                ui.add(
                                    egui::Slider::new(&mut t.opacity, 0.0..=1.0)
                                        .text(text(Text::Opacity)),
                                );
                                ui.add_enabled(
                                    item.blob().is_some(),
                                    egui::Checkbox::new(&mut t.grayscale, text(Text::Grayscale)),
                                );
                                ui.add_enabled(
                                    item.blob().is_some(),
                                    egui::Checkbox::new(&mut t.flip[0], text(Text::FlipHorizontal)),
                                );
                                ui.add_enabled(
                                    item.blob().is_some(),
                                    egui::Checkbox::new(&mut t.flip[1], text(Text::FlipVertical)),
                                );
                                ui.add_enabled_ui(item.blob().is_some(), |ui| {
                                    ui.collapsing(text(Text::Content), |ui| {
                                        number(
                                            ui,
                                            Text::ContentScale,
                                            &mut t.content_scale,
                                            0.001..=1000.,
                                        );
                                        number(
                                            ui,
                                            Text::OffsetX,
                                            &mut t.content_offset[0],
                                            -1000.0..=1000.0,
                                        );
                                        number(
                                            ui,
                                            Text::OffsetY,
                                            &mut t.content_offset[1],
                                            -1000.0..=1000.0,
                                        );
                                        let mut degrees = t.content_rotation.to_degrees();
                                        number(
                                            ui,
                                            Text::Rotation,
                                            &mut degrees,
                                            -36000.0..=36000.0,
                                        );
                                        t.content_rotation = degrees.to_radians();
                                    });
                                });
                                if ui
                                    .add_enabled(
                                        frame.valid() && t.valid(),
                                        egui::Button::new(moth_ui::tooltip(ui.ctx(), Text::Apply)),
                                    )
                                    .clicked()
                                {
                                    if *frame != item.rect {
                                        commands.push(Command::SetRect {
                                            id: *id,
                                            rect: *frame,
                                        });
                                    }
                                    if *t != item.transform {
                                        commands.push(Command::SetTransform {
                                            id: *id,
                                            transform: *t,
                                        });
                                    }
                                }
                                if ui
                                    .button(moth_ui::tooltip(ui.ctx(), Text::Cancel))
                                    .clicked()
                                {
                                    *frame = item.rect;
                                    *t = item.transform;
                                }
                            });
                        } else {
                            self.draft = None;
                            self.observed = None;
                        }
                        ui.collapsing(text(Text::Arrange), |ui| {
                            let old_gap = self.gap;
                            number(ui, Text::Gap, &mut self.gap, 1.0..=10000.);
                            ui.small(text(Text::GapHint));
                            egui::ComboBox::from_id_salt("arrange_order")
                                .selected_text(text(order_label(self.order)))
                                .show_ui(ui, |ui| {
                                    for i in 0..4 {
                                        ui.selectable_value(
                                            &mut self.order,
                                            i,
                                            text(order_label(i)),
                                        );
                                    }
                                });
                            for i in 0..4 {
                                let (key, mode) = layout(i);
                                if i < 2 {
                                    let count = &mut self.layout_counts[mode as usize];
                                    *count = (*count).clamp(1, items.len().max(1));
                                    ui.add(egui::Slider::new(count, 1..=items.len().max(1)).text(
                                        text(if i == 0 {
                                            Text::LayoutRows
                                        } else {
                                            Text::LayoutColumns
                                        }),
                                    ));
                                    ui.small(text(if i == 0 {
                                        Text::HorizontalHint
                                    } else {
                                        Text::VerticalHint
                                    }));
                                }
                                if i == 2 {
                                    ui.small(text(Text::GridHint));
                                }
                                if ui
                                    .add_enabled_ui(!items.is_empty(), |ui| {
                                        moth_ui::icons::labeled_button(
                                            ui,
                                            layout_icon(mode),
                                            &moth_ui::tooltip(ui.ctx(), key),
                                            false,
                                        )
                                        .on_hover_text(
                                            text(if i == 3 {
                                                Text::OptimalHint
                                            } else {
                                                Text::GapHint
                                            }),
                                        )
                                    })
                                    .inner
                                    .clicked()
                                {
                                    commands.extend(self.arrange(&mut items, mode));
                                }
                            }
                            if old_gap != self.gap {
                                intents.push(Intent::PreferencesDirty);
                            }
                            if moth_ui::icons::labeled_button(
                                ui,
                                moth_ui::icons::Icon::ArrangeOptimal,
                                &moth_ui::tooltip(ui.ctx(), Text::PackFit),
                                false,
                            )
                            .clicked()
                            {
                                let mut all: Vec<_> = board.scene.items().collect();
                                all.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                                commands.extend(arrange::arrange(
                                    &all,
                                    arrange::Arrange::Optimal,
                                    self.gap,
                                    arrange::FreeRectChoice::from_index(self.arrange_choice),
                                    arrange::GuillotineSplit::from_index(self.arrange_split),
                                ));
                                intents.push(Intent::NeedsFit);
                            }
                            for (label, mode) in [
                                (Text::NormalizeWidth, arrange::Normalize::Width),
                                (Text::NormalizeHeight, arrange::Normalize::Height),
                                (Text::NormalizeSize, arrange::Normalize::Size),
                                (Text::NormalizeArea, arrange::Normalize::Area),
                            ] {
                                if ui.button(moth_ui::tooltip(ui.ctx(), label)).clicked() {
                                    commands.extend(arrange::normalize(&items, mode));
                                }
                            }
                            for (label, mode) in [
                                (Text::AlignLeft, arrange::Align::Left),
                                (Text::AlignRight, arrange::Align::Right),
                                (Text::AlignTop, arrange::Align::Top),
                                (Text::AlignBottom, arrange::Align::Bottom),
                                (Text::DistributeHorizontal, arrange::Align::Horizontal),
                                (Text::DistributeVertical, arrange::Align::Vertical),
                                (Text::Stack, arrange::Align::Stack),
                            ] {
                                if ui.button(moth_ui::tooltip(ui.ctx(), label)).clicked() {
                                    commands.extend(arrange::align(&items, mode));
                                }
                            }
                            number(ui, Text::ScaleSelection, &mut self.factor, 0.001..=1000.);
                            if ui
                                .add_enabled(
                                    !items.is_empty(),
                                    egui::Button::new(moth_ui::tooltip(ui.ctx(), Text::Apply)),
                                )
                                .clicked()
                            {
                                commands.extend(scale(&items, self.factor));
                            }
                        });
                        for (key, mode) in [
                            (Text::ResetScale, Geometry::Scale),
                            (Text::NormalizeScale, Geometry::Normalize),
                        ] {
                            if ui.button(moth_ui::tooltip(ui.ctx(), key)).clicked() {
                                intents.push(Intent::OriginalGeometry(
                                    items
                                        .iter()
                                        .filter(|i| i.blob().is_some())
                                        .map(|i| i.id)
                                        .collect(),
                                    mode,
                                ));
                            }
                        }
                        ui.horizontal_wrapped(|ui| {
                            for (key, delta, duplicate) in [
                                (Text::Duplicate, [24., 24.], true),
                                (Text::NudgeLeft, [-1., 0.], false),
                                (Text::NudgeRight, [1., 0.], false),
                                (Text::NudgeUp, [0., -1.], false),
                                (Text::NudgeDown, [0., 1.], false),
                            ] {
                                if ui
                                    .add_enabled(
                                        !board.selection.is_empty(),
                                        egui::Button::new(moth_ui::tooltip(ui.ctx(), key)),
                                    )
                                    .clicked()
                                {
                                    commands.extend(arrange::translate(
                                        &items,
                                        delta,
                                        duplicate,
                                        board.scene.top(),
                                    ));
                                }
                            }
                        });
                        for (key, front, extreme) in [
                            (Text::BringFront, true, true),
                            (Text::SendBack, false, true),
                            (Text::BringForward, true, false),
                            (Text::SendBackward, false, false),
                        ] {
                            if ui.button(moth_ui::tooltip(ui.ctx(), key)).clicked() {
                                commands.extend(arrange::reorder(
                                    &board.scene,
                                    &items.iter().map(|i| i.id).collect(),
                                    front,
                                    extreme,
                                ));
                            }
                        }
                        for (key, kind) in [
                            (Text::ResetRotation, 0),
                            (Text::ResetFlip, 1),
                            (Text::ResetCrop, 2),
                            (Text::ResetTransform, 3),
                        ] {
                            if ui.button(moth_ui::tooltip(ui.ctx(), key)).clicked() {
                                if kind >= 2 {
                                    intents.push(Intent::OriginalGeometry(
                                        items.iter().map(|i| i.id).collect(),
                                        if kind == 2 {
                                            Geometry::Crop
                                        } else {
                                            Geometry::All
                                        },
                                    ));
                                    continue;
                                }
                                for item in &items {
                                    let mut transform = item.transform;
                                    match kind {
                                        0 => transform.rotation = 0.,
                                        1 => transform.flip = [false; 2],
                                        2 => {
                                            transform.content_scale = 1.;
                                            transform.content_offset = [0.; 2];
                                            transform.content_rotation = 0.;
                                        }
                                        _ => transform = transform.reset_geometry(),
                                    };
                                    commands.push(Command::SetTransform {
                                        id: item.id,
                                        transform,
                                    });
                                }
                            }
                        }
                    });
                    ui.checkbox(&mut self.picking, text(Text::SampleColor));
                    ui.horizontal(|ui| {
                        for (i, label) in ["HEX", "RGB", "HSV"].into_iter().enumerate() {
                            ui.selectable_value(&mut self.color_format, i as u8, label);
                        }
                    });
                    ui.checkbox(&mut self.coordinates, text(Text::CoordinatePicker));
                    ui.collapsing(text(Text::Export), |ui| {
                        ui.label(text(Text::ExportLimit));
                        ui.label(text(Text::Path));
                        ui.text_edit_singleline(&mut self.path);
                        ui.horizontal(|ui| {
                            ui.label(text(Text::ExportSide));
                            ui.add(egui::DragValue::new(&mut self.side).range(1..=4096));
                        });
                        ui.checkbox(&mut self.crop, text(Text::CropRegion));
                        if self.crop {
                            rect(ui, &mut self.region);
                        }
                        let path = std::path::PathBuf::from(&self.path);
                        let valid = !self.path.is_empty() && (!self.crop || self.region.valid());
                        if ui
                            .add_enabled(
                                valid && !board.scene.is_empty(),
                                egui::Button::new(moth_ui::tooltip(ui.ctx(), Text::ExportScene)),
                            )
                            .clicked()
                        {
                            export = Some(Target::Scene {
                                path: path.clone(),
                                ids: Vec::new(),
                                region: self.crop.then_some(self.region),
                                side: self.side,
                            });
                        }
                        if ui
                            .add_enabled(
                                valid && !items.is_empty(),
                                egui::Button::new(moth_ui::tooltip(
                                    ui.ctx(),
                                    Text::ExportSelection,
                                )),
                            )
                            .clicked()
                        {
                            export = Some(Target::Scene {
                                path: path.clone(),
                                ids: items.iter().map(|i| i.id).collect(),
                                region: self.crop.then_some(self.region),
                                side: self.side,
                            });
                        }
                        if let [item] = items.as_slice()
                            && item.blob().is_some()
                        {
                            if ui
                                .add_enabled(
                                    valid,
                                    egui::Button::new(moth_ui::tooltip(
                                        ui.ctx(),
                                        Text::ExportOriginal,
                                    )),
                                )
                                .clicked()
                            {
                                export = Some(Target::Original {
                                    path: path.clone(),
                                    id: item.id,
                                });
                            }
                            if ui
                                .button(moth_ui::tooltip(ui.ctx(), Text::CopyImage))
                                .clicked()
                            {
                                export = Some(Target::Clipboard(item.id));
                            }
                        }
                        if ui
                            .add_enabled(
                                valid && !board.scene.is_empty(),
                                egui::Button::new(moth_ui::tooltip(
                                    ui.ctx(),
                                    Text::ExportOriginals,
                                )),
                            )
                            .clicked()
                        {
                            export = Some(Target::Originals(path.clone()));
                        }
                        if ui
                            .add_enabled(
                                valid && !board.scene.is_empty(),
                                egui::Button::new(moth_ui::tooltip(ui.ctx(), Text::ExportZip)),
                            )
                            .clicked()
                        {
                            export = Some(Target::Archive {
                                path,
                                ids: items.iter().map(|i| i.id).collect(),
                            });
                        }
                    });
                });
            });
        self.open = open;
        (commands, export, note, intents)
    }
}
pub(super) fn layout(i: u8) -> (Text, arrange::Arrange) {
    match i {
        1 => (Text::Vertical, arrange::Arrange::Vertical),
        2 => (Text::Grid, arrange::Arrange::Grid),
        3 => (Text::Optimal, arrange::Arrange::Optimal),
        _ => (Text::Horizontal, arrange::Arrange::Horizontal),
    }
}
pub(super) fn layout_icon(mode: arrange::Arrange) -> moth_ui::icons::Icon {
    use moth_ui::icons::Icon;
    match mode {
        arrange::Arrange::Horizontal => Icon::ArrangeHorizontal,
        arrange::Arrange::Vertical => Icon::ArrangeVertical,
        arrange::Arrange::Grid => Icon::ArrangeGrid,
        arrange::Arrange::Optimal => Icon::ArrangeOptimal,
    }
}
fn order_label(i: u8) -> Text {
    match i {
        1 => Text::ByFilename,
        2 => Text::ByTime,
        3 => Text::Random,
        _ => Text::ByLayer,
    }
}
fn choice_label(i: u8) -> Text {
    match i {
        0 => Text::PackBestArea,
        1 => Text::PackBestShortSide,
        2 => Text::PackBestLongSide,
        3 => Text::PackWorstArea,
        4 => Text::PackWorstShortSide,
        _ => Text::PackWorstLongSide,
    }
}
fn split_label(i: u8) -> Text {
    match i {
        0 => Text::SplitShorterLeftover,
        1 => Text::SplitLongerLeftover,
        2 => Text::SplitMinimizeArea,
        3 => Text::SplitMaximizeArea,
        4 => Text::SplitShorterAxis,
        _ => Text::SplitLongerAxis,
    }
}
fn scale(items: &[&moth_scene::Item], factor: f32) -> Vec<Command> {
    if !factor.is_finite() || factor <= 0. {
        return Vec::new();
    }
    let Some(first) = items.first() else {
        return Vec::new();
    };
    let commands: Vec<_> = items
        .iter()
        .map(|item| Command::SetRect {
            id: item.id,
            rect: Rect {
                x: first.rect.x + (item.rect.x - first.rect.x) * factor,
                y: first.rect.y + (item.rect.y - first.rect.y) * factor,
                width: item.rect.width * factor,
                height: item.rect.height * factor,
            },
        })
        .collect();
    if commands
        .iter()
        .all(|c| matches!(c,Command::SetRect{rect,..} if rect.valid()))
    {
        commands
    } else {
        Vec::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_zero_gap_restores_gutters_without_spacing_optimal() {
        let mut tools = Tools::default();
        tools.load(&[("arrange_gap".into(), "0".into())].into_iter().collect());
        let items: Vec<_> = (0..4)
            .map(|_| {
                moth_scene::Item::image(
                    moth_scene::BlobId::from_bytes([0; 32]),
                    Rect {
                        x: 0.,
                        y: 0.,
                        width: 100.,
                        height: 100.,
                    },
                    None,
                )
            })
            .collect();
        let mut refs: Vec<_> = items.iter().collect();
        for mode in [arrange::Arrange::Grid, arrange::Arrange::Optimal] {
            let mut rects: Vec<_> = tools
                .arrange(&mut refs, mode)
                .into_iter()
                .map(|c| {
                    let Command::SetRect { rect, .. } = c else {
                        panic!("expected position command")
                    };
                    rect
                })
                .collect();
            assert_eq!(rects.len(), 4);
            rects.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
            let expected_gap = if matches!(mode, arrange::Arrange::Grid) {
                24.
            } else {
                0.
            };
            assert_eq!(rects[2].x - rects[0].x - rects[0].width, expected_gap);
            assert_eq!(rects[1].y - rects[0].y - rects[0].height, expected_gap);
        }
        assert_eq!(tools.gap, 24.);
    }
    #[test]
    fn repeated_arrangement_reverses_the_same_order_without_moving_the_anchor() {
        let a = moth_scene::Item::text(
            "a",
            Rect {
                x: 10.,
                y: 20.,
                width: 10.,
                height: 10.,
            },
            None,
        );
        let b = moth_scene::Item::text(
            "b",
            Rect {
                x: 30.,
                y: 20.,
                width: 20.,
                height: 10.,
            },
            Some(&a.z),
        );
        for order in [0, 1, 2, 3] {
            let mut tools = Tools {
                order,
                ..Tools::default()
            };
            let mut items = vec![&a, &b];
            let first = tools.arrange(&mut items, arrange::Arrange::Horizontal);
            let reversed = tools.arrange(&mut items, arrange::Arrange::Horizontal);
            assert!(
                matches!((&first[0],&reversed[1]),(Command::SetRect{id:a,..},Command::SetRect{id:b,..}) if a==b)
            );
            assert!(
                matches!((&first[0],&reversed[0]),(Command::SetRect{rect:a,..},Command::SetRect{rect:b,..}) if a.x==b.x && a.y==b.y)
            );
        }
    }
    #[test]
    fn video_limits_default_to_the_adapter_and_ignore_legacy_caps() {
        let mut tools = Tools::default();
        for (saved, expected) in [
            ("0", 0),
            ("256", 256),
            ("12288", 12288),
            ("255", 0),
            ("x", 0),
        ] {
            tools.load(
                &[
                    ("vram_limit_mib".into(), saved.into()),
                    ("cache_limit_mib".into(), "512".into()),
                    ("cache_ram_percent".into(), "50".into()),
                ]
                .into_iter()
                .collect(),
            );
            assert_eq!(tools.vram_limit_mib, expected);
            assert!(
                !tools
                    .preferences()
                    .iter()
                    .any(|(key, _)| *key == "cache_ram_percent" || *key == "cache_limit_mib")
            );
        }
        let video = moth_platform::gpu::VideoMemory {
            name: "Test GPU".into(),
            kind: moth_platform::gpu::AdapterKind::Discrete,
            dedicated: Some(8 << 30),
            system: Some(16 << 30),
            limit: 6 << 30,
        };
        assert!(video_summary(&video).contains("Test GPU"));
        assert!(video_summary(&video).contains("6.0 GiB"));
    }
    #[test]
    fn preferences_roundtrip_and_reject_nonfinite_values() {
        let source = Tools {
            shortcut_feedback: false,
            dark: false,
            ui_scale: 1.5,
            gap: 17.,
            smooth_zoom: false,
            zoom_increment: 25.,
            threshold: 9,
            auto_arrange: true,
            default_arrange: 3,
            undo_limit: 42,
            vram_limit_mib: 4096,
            performance_mode: moth_io::CacheMode::Disk,
            bake_mode: moth_io::BakeMode::Board,
            show_outlines: false,
            drag_animation: false,
            drag_opacity: 0.42,
            ..Tools::default()
        };
        let map = source
            .preferences()
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        let mut loaded = Tools::default();
        loaded.load(&map);
        assert_eq!(loaded.preferences(), source.preferences());
        loaded.load(
            &[
                ("ui_scale".into(), "NaN".into()),
                ("zoom_increment".into(), "NaN".into()),
                ("drag_opacity".into(), "NaN".into()),
                ("drag_animation".into(), "invalid".into()),
                ("show_outlines".into(), "invalid".into()),
                ("arrange_gap".into(), "inf".into()),
                ("undo_limit".into(), "99999".into()),
                ("vram_limit_mib".into(), "99999999999".into()),
                ("performance_mode".into(), "bogus".into()),
                ("bake_mode".into(), "bogus".into()),
            ]
            .into_iter()
            .collect(),
        );
        assert!(loaded.show_outlines);
        assert!(loaded.drag_animation);
        assert_eq!(
            loaded.drag_opacity,
            super::super::motion::DEFAULT_DRAG_OPACITY
        );
        assert_eq!(loaded.zoom_increment, 10.);
        assert_eq!(loaded.ui_scale, 1.);
        assert_eq!(loaded.gap, 24.);
        assert_eq!(loaded.undo_limit, 256);
        assert_eq!(loaded.vram_limit_mib, 0);
        assert_eq!(loaded.performance_mode, moth_io::CacheMode::Full);
        assert_eq!(loaded.bake_mode, moth_io::BakeMode::Visible);
    }
    #[test]
    fn scaling_rejects_overflow_and_keeps_relative_positions() {
        let a = moth_scene::Item::text(
            "a",
            Rect {
                x: 10.,
                y: 20.,
                width: 10.,
                height: 10.,
            },
            None,
        );
        let mut b = a.clone();
        b.id = Uuid::now_v7();
        b.rect.x = 30.;
        let out = scale(&[&a, &b], 2.);
        assert!(matches!(out[1],Command::SetRect{rect,..} if rect.x==50.&&rect.width==20.));
        assert!(scale(&[&a], f32::INFINITY).is_empty());
        assert!(scale(&[&a], 1e9).is_empty());
    }
}
