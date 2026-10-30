// SPDX-License-Identifier: GPL-3.0-or-later
//! What a drawing session asks of the app: the window (full screen or a small window
//! on top that follows the image's aspect ratio), sounds, remembered setups and the
//! way back to the board.
use super::*;
use session_ui::Event;

/// Window state before a session took it over.
pub(super) struct SavedWindow {
    size: winit::dpi::PhysicalSize<u32>,
    position: Option<winit::dpi::PhysicalPosition<i32>>,
    maximized: bool,
    fullscreen: bool,
    pinned: bool,
}
/// Longer side of the mini window in logical points when it opens.
const MINI_SIDE: f64 = 360.;

impl App {
    /// Slides a running session or its summary draws instead of the board: copies of
    /// board items placed in screen points, their clip and the stage color.
    pub(super) fn session_stage(&self) -> Option<(Vec<moth_scene::Item>, egui::Rect, wgpu::Color)> {
        let state = self
            .workspace
            .board
            .session
            .as_ref()
            .filter(|s| s.covers_board())?;
        let scene = &self.workspace.board.scene;
        let items = state
            .draws
            .iter()
            .filter_map(|(id, target)| {
                let mut item = scene.get(*id)?.clone();
                let envelope = item.rect.rotated_envelope(item.transform.rotation);
                let width = envelope.upper()[0] - envelope.lower()[0];
                let scale = target.width() / width.max(f32::EPSILON);
                item.rect = moth_scene::Rect {
                    x: target.center().x - item.rect.width * scale / 2.,
                    y: target.center().y - item.rect.height * scale / 2.,
                    width: item.rect.width * scale,
                    height: item.rect.height * scale,
                };
                item.rect.valid().then_some(item)
            })
            .collect();
        let ctx = self.shell.context();
        let [r, g, b, _] = moth_ui::tokens(ctx).theater.to_array();
        let color = wgpu::Color {
            r: r as f64 / 255.,
            g: g as f64 / 255.,
            b: b as f64 / 255.,
            a: 1.,
        };
        Some((items, state.clip.unwrap_or(ctx.content_rect()), color))
    }
    /// Plans under "My plans": built-ins, then the user's.
    pub(super) fn session_plans(&self) -> Vec<(String, Vec<session::Stage>)> {
        let mut plans = session_ui::builtin_plans();
        if let Some(saved) = self.window_preferences.get("session_plans") {
            for plan in saved.split(';').filter_map(session::decode_plan) {
                if !plan.is_empty() && !plans.iter().any(|(_, p)| *p == plan) {
                    let name = plan
                        .iter()
                        .map(|s| format!("{}×{}", s.count, session_ui::seconds(s.seconds as u64)))
                        .collect::<Vec<_>>()
                        .join(" + ");
                    plans.push((name, plan));
                }
            }
        }
        plans
    }
    pub(super) fn session_event(&mut self, event: Event) {
        match event {
            Event::Window { mini, aspect } => self.session_window(mini, aspect),
            Event::Leave => self.restore_window(),
            Event::Cue(cue) => moth_platform::sound::play(match cue {
                session::Cue::Change => moth_platform::sound::Sound::Chime,
                session::Cue::Tick => moth_platform::sound::Sound::Tick,
            }),
            Event::Select(ids) => {
                self.restore_window();
                let board = &mut self.workspace.board;
                board.entered = None;
                board.selection = ids
                    .into_iter()
                    .filter(|id| board.scene.get(*id).is_some())
                    .collect();
                let selection = board.selection.clone();
                board.view.fit_selection(&board.scene, &selection);
            }
            Event::Remember(setup) => {
                self.window_preferences
                    .insert("session_setup".into(), setup.encode());
                self.window_preferences_dirty = true;
            }
            Event::SavePlan(plan) => {
                let mut saved: Vec<String> = self
                    .window_preferences
                    .get("session_plans")
                    .map(|value| value.split(';').map(str::to_owned).collect())
                    .unwrap_or_default();
                let encoded = session::encode_plan(&plan);
                if !encoded.is_empty() && !saved.contains(&encoded) {
                    saved.push(encoded);
                    // A short list stays a list of favourites, not a history.
                    let excess = saved.len().saturating_sub(8);
                    saved.drain(..excess);
                    self.window_preferences
                        .insert("session_plans".into(), saved.join(";"));
                    self.window_preferences_dirty = true;
                }
            }
            Event::Drag => self.drag_window(None),
        }
    }
    /// Takes the window over: full screen, or a small window on top whose shape follows
    /// the image while its top-left corner stays put.
    fn session_window(&mut self, mini: bool, aspect: f32) {
        let Some(window) = self.shell.window.clone() else {
            return;
        };
        if self.session_window.is_none() {
            self.session_window = Some(SavedWindow {
                size: window.inner_size(),
                position: window.outer_position().ok(),
                maximized: window.is_maximized(),
                fullscreen: window.fullscreen().is_some(),
                pinned: self.pinned,
            });
        }
        if !mini {
            if window.fullscreen().is_none() {
                window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            }
            return;
        }
        if window.fullscreen().is_some() {
            window.set_fullscreen(None);
        }
        if window.is_maximized() {
            window.set_maximized(false);
        }
        let aspect = f64::from(aspect).clamp(0.2, 5.);
        let scale = window.scale_factor();
        let current = window.inner_size().to_logical::<f64>(scale);
        // Keep the user's chosen size along the longer side once the window is small.
        let side = if current.width.max(current.height) <= MINI_SIDE * 2. {
            current.width.max(current.height)
        } else {
            MINI_SIDE
        };
        let size = if aspect >= 1. {
            winit::dpi::LogicalSize::new(side, side / aspect)
        } else {
            winit::dpi::LogicalSize::new(side * aspect, side)
        };
        let applied = window.request_inner_size(size);
        self.resized(applied);
        if !self.pinned {
            self.apply_pin(true);
        }
    }
    /// Wayland applies a requested size at once and sends no `Resized` event for it,
    /// so the surface follows here.
    pub(super) fn resized(&mut self, applied: Option<winit::dpi::PhysicalSize<u32>>) {
        if let (Some(size), Some(gpu)) = (applied, &mut self.shell.gpu) {
            gpu.resize(size.width, size.height);
            self.redraw();
        }
    }
    /// Gives the window back as it was before the session.
    pub(super) fn restore_window(&mut self) {
        let Some(saved) = self.session_window.take() else {
            return;
        };
        let Some(window) = self.shell.window.clone() else {
            return;
        };
        window.set_fullscreen(
            saved
                .fullscreen
                .then_some(winit::window::Fullscreen::Borderless(None)),
        );
        // The old size first, so a compositor that ignores the maximize request
        // still gives the window back at its previous size.
        let applied = window.request_inner_size(saved.size);
        self.resized(applied);
        if let Some(position) = saved.position {
            window.set_outer_position(position);
        }
        if saved.maximized {
            window.set_maximized(true);
        }
        if self.pinned != saved.pinned {
            self.apply_pin(saved.pinned);
        }
    }
}
