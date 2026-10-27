// SPDX-License-Identifier: GPL-3.0-or-later
//! Presentation belongs to a board tab, never to its scene or undo history.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Appearance {
    pub view: bool,
    pub over: bool,
    pub background: u8,
    pub images: f32,
    pub canvas: f32,
    pub window: f32,
    pub images_only: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            view: false,
            over: false,
            background: 0,
            images_only: true,
            images: 1.,
            canvas: 0.55,
            window: 1.,
        }
    }
}
impl Appearance {
    pub fn compact(self, width: f32) -> bool {
        self.view || width < 600.
    }
    pub fn presents(self, compact: bool, item: &Item) -> bool {
        !compact || !self.images_only || item.blob().is_some()
    }
    pub fn encode(self) -> String {
        format!(
            "{},{},{},{},{},{},{}",
            u8::from(self.view),
            self.background,
            self.images,
            self.canvas,
            self.window,
            u8::from(self.images_only),
            u8::from(self.over)
        )
    }
    pub fn decode(value: &str) -> Option<Self> {
        let fields: Vec<_> = value.split(',').collect();
        if fields.len() != 7 {
            return None;
        }
        let flag = |v| match v {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        };
        let opacity = |v: &str| {
            v.parse::<f32>()
                .ok()
                .filter(|v| v.is_finite() && (0.0..=1.).contains(v))
        };
        let background = fields[1].parse::<u8>().ok().filter(|v| *v <= 2)?;
        Some(Self {
            view: flag(fields[0])?,
            over: flag(fields[6])?,
            background,
            images: opacity(fields[2])?,
            canvas: opacity(fields[3])?,
            window: opacity(fields[4]).filter(|v| *v >= 0.1)?,
            images_only: flag(fields[5])?,
        })
    }
}

pub(super) fn key(path: &str) -> Option<String> {
    moth_platform::settings::board_window_key(path)
}

/// Shared controls in the View popover and the independent Over controller.
pub(super) fn controls(ui: &mut egui::Ui, appearance: &mut Appearance, blur: bool) {
    ui.vertical(|ui| {
        use moth_ui::{Text, text};
        ui.add(egui::Slider::new(&mut appearance.images, 0.0..=1.).text(text(Text::ImageOpacity)));
        ui.add_enabled(
            appearance.background != 0,
            egui::Slider::new(&mut appearance.canvas, 0.0..=1.).text(text(Text::BoardOpacity)),
        );
        ui.add(egui::Slider::new(&mut appearance.window, 0.1..=1.).text(text(Text::WindowOpacity)));
        egui::ComboBox::from_id_salt("board_background")
            .selected_text(text(
                [
                    Text::BackgroundSolid,
                    Text::BackgroundTranslucent,
                    Text::BackgroundGlass,
                ][appearance.background as usize],
            ))
            .show_ui(ui, |ui| {
                for (value, label) in [
                    Text::BackgroundSolid,
                    Text::BackgroundTranslucent,
                    Text::BackgroundGlass,
                ]
                .into_iter()
                .enumerate()
                {
                    ui.selectable_value(&mut appearance.background, value as u8, text(label));
                }
            });
        if appearance.background == 2 && !blur {
            ui.label(text(Text::GlassFallback));
        }
        ui.checkbox(&mut appearance.images_only, text(Text::ImagesOnly));
    });
}

impl App {
    pub(super) fn remember_appearance(&mut self) {
        if self.workspace.board.loading {
            return;
        }
        if let Some(key) = key(&self.workspace.board.path) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let value = format!("{now}|{}", self.workspace.board.appearance.encode());
            self.window_preferences.insert(key, value);
            moth_platform::settings::trim_board_windows(&mut self.window_preferences);
            self.window_preferences_dirty = true;
        }
    }
    pub(super) fn flush_appearance(&mut self) {
        if self.window_preferences_dirty
            && !self.shell.context().input(|i| i.pointer.any_down())
            && self
                .workspace
                .board
                .boards()
                .requests
                .try_send(moth_platform::boards::Request::Preferences(
                    self.window_preferences.clone(),
                ))
                .is_ok()
        {
            self.workspace.board.in_flight += 1;
            self.window_preferences_dirty = false;
        }
    }
    pub(super) fn restore_appearance(&mut self) {
        self.workspace.board.appearance = key(&self.workspace.board.path)
            .and_then(|k| self.window_preferences.get(&k))
            .and_then(|v| v.split_once('|'))
            .and_then(|(_, v)| Appearance::decode(v))
            .unwrap_or_default();
    }
    pub(super) fn compact(&self) -> bool {
        !self.workspace.library
            && (self.overlay.is_some()
                || self.workspace.board.appearance.compact(
                    self.shell.window.as_ref().map_or(1000., |w| {
                        w.inner_size().width as f32 / self.shell.context().pixels_per_point()
                    }),
                ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appearance_controls_stack_inside_a_horizontal_surface() {
        let ctx = egui::Context::default();
        let mut appearance = Appearance {
            background: 2,
            ..Appearance::default()
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(530., 220.),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let available = ui.available_rect_before_wrap();
                    let response = ui.horizontal_wrapped(|ui| {
                        controls(ui, &mut appearance, false);
                    });
                    assert!(response.response.rect.right() <= available.right());
                    assert!(response.response.rect.height() > ui.spacing().interact_size.y * 4.);
                });
            },
        );
        output.textures_delta.clear();
    }

    #[test]
    fn appearance_round_trip_and_validation() {
        let value = Appearance {
            view: true,
            background: 2,
            images: 0.4,
            images_only: true,
            ..Appearance::default()
        };
        assert_eq!(Appearance::decode(&value.encode()), Some(value));
        for bad in [
            "1,3,1,1,1,0,0",
            "1,0,NaN,1,1,0,0",
            "1,0,-1,1,1,0,0",
            "1,0,1,1,0,0,0",
            "2,0,1,1,1,0,0",
            "1,0,1,1,1,0,2",
            "1,0,1,1,1,0,extra",
        ] {
            assert!(Appearance::decode(bad).is_none());
        }
        assert!(Appearance::default().compact(599.));
        assert!(!Appearance::default().compact(600.));
        assert!(value.compact(1000.));
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 10.,
            height: 10.,
        };
        let note = Item::text("Reference note", rect, None);
        let image = Item::image(BlobId::from_bytes([0; 32]), rect, None);
        assert!(Appearance::default().presents(false, &note));
        assert!(!Appearance::default().presents(true, &note));
        assert!(Appearance::default().presents(true, &image));
        assert!(
            Appearance {
                images_only: false,
                ..Appearance::default()
            }
            .presents(true, &note)
        );
        assert!(key("").is_none());
        assert_ne!(key("one.moth"), key("two.moth"));
    }
}

/// Quantized steps avoid floating-point drift at the visible endpoints.
pub(super) fn adjust_opacity(value: f32, increase: bool) -> f32 {
    ((value * 10.).round() + if increase { 1. } else { -1. }).clamp(1., 10.) / 10.
}
impl App {
    pub(super) fn window_position_locked(&self) -> bool {
        self.locked_position.is_some()
            && self.shell.window.as_ref().is_some_and(|w| {
                w.fullscreen().is_none() && (!w.is_maximized() || self.settings.lock_maximized)
            })
    }
    pub(super) fn resize_to_items(&mut self, selected: bool) {
        let bounds = self
            .workspace
            .board
            .scene
            .items()
            .filter(|item| !selected || self.workspace.board.selection.contains(&item.id))
            .map(|item| {
                let e = item.rect.rotated_envelope(item.transform.rotation);
                Rect {
                    x: e.lower()[0],
                    y: e.lower()[1],
                    width: e.upper()[0] - e.lower()[0],
                    height: e.upper()[1] - e.lower()[1],
                }
            })
            .reduce(|a, b| Rect {
                x: a.x.min(b.x),
                y: a.y.min(b.y),
                width: (a.x + a.width).max(b.x + b.width) - a.x.min(b.x),
                height: (a.y + a.height).max(b.y + b.height) - a.y.min(b.y),
            });
        let (Some(bounds), Some(window)) = (bounds, self.shell.window.as_ref()) else {
            return;
        };
        let scale = self.shell.context().pixels_per_point();
        let size = window.inner_size();
        let viewport = self.workspace.board.view.safe_area;
        if !viewport.is_positive() {
            return;
        }
        let chrome = [
            size.width as f32 / scale - viewport.width(),
            size.height as f32 / scale - viewport.height(),
        ];
        let padding = moth_ui::tokens(self.shell.context()).space_xl as f32 * 2.;
        let zoom = self.workspace.board.view.camera.zoom;
        let maximum = window.current_monitor().map(|m| m.size()).unwrap_or(size);
        let target = winit::dpi::PhysicalSize::new(
            ((bounds.width * zoom + chrome[0] + padding) * scale).clamp(1., maximum.width as f32)
                as u32,
            ((bounds.height * zoom + chrome[1] + padding) * scale).clamp(1., maximum.height as f32)
                as u32,
        );
        self.camera_motion.stop();
        self.resize_bounds = Some(bounds);
        let _ = window.request_inner_size(target);
        self.redraw();
    }
}
#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn opacity_steps_clamp_and_reverse() {
        assert_eq!(adjust_opacity(0.1, false), 0.1);
        assert_eq!(adjust_opacity(1., true), 1.);
        assert_eq!(adjust_opacity(adjust_opacity(0.5, true), false), 0.5);
    }
}

impl App {
    pub(super) fn configure_global_shortcuts(&mut self) {
        if !self.settings.global_shortcuts {
            self.global_shortcuts = None;
            return;
        }
        if self.global_shortcuts.is_some() {
            return;
        }
        let Some(window) = &self.shell.window else {
            return;
        };
        if !moth_platform::Desktop.global_shortcuts_available(window) {
            self.settings.global_shortcuts = false;
            self.workspace.board.message = moth_ui::Text::GlobalShortcutsUnavailable.into();
            return;
        }
        match moth_platform::shortcuts::Shortcuts::new(window, self.shell.shortcut_wake()) {
            Ok(keys) => self.global_shortcuts = Some(keys),
            Err(error) => {
                self.settings.global_shortcuts = false;
                self.workspace.board.message = error.into();
            }
        }
    }
}
