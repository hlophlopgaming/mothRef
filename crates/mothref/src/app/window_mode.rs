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
