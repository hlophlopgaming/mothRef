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
    /// The session currently holds the window. The saved state outlives a restore so
    /// a repeat or a new session from the summary never mistakes the mini window for
    /// the user's own size, even when the restore is still being applied.
    taken: bool,
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
    pub(super) fn session_plans(&self) -> Vec<session_ui::Plan> {
        let mut plans = session_ui::builtin_plans();
        for (name, stages) in self.saved_plans() {
            if !stages.is_empty() {
                plans.push(session_ui::Plan {
                    name,
                    stages,
                    saved: true,
                });
            }
        }
        plans
    }
    /// `name=10x30,5x120;…`; entries saved before plans had names get one from their stages.
    fn saved_plans(&self) -> Vec<(String, Vec<session::Stage>)> {
        let Some(saved) = self.window_preferences.get("session_plans") else {
            return Vec::new();
        };
        saved
            .split(';')
            .filter_map(|entry| {
                let (name, plan) = entry.split_once('=').unwrap_or(("", entry));
                let stages = session::decode_plan(plan)?;
                let name = if name.is_empty() {
                    stages
                        .iter()
                        .map(|s| format!("{}×{}", s.count, session_ui::seconds(s.seconds as u64)))
                        .collect::<Vec<_>>()
                        .join(" + ")
                } else {
                    name.to_owned()
                };
                Some((name, stages))
            })
            .collect()
    }
    fn store_plans(&mut self, plans: &[(String, Vec<session::Stage>)]) {
        let value = plans
            .iter()
            .map(|(name, stages)| format!("{name}={}", session::encode_plan(stages)))
            .collect::<Vec<_>>()
            .join(";");
        self.window_preferences
            .insert("session_plans".into(), value);
        self.window_preferences_dirty = true;
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
            Event::SavePlan(name, plan) => {
                if plan.is_empty() || name.is_empty() {
                    return;
                }
                let mut saved = self.saved_plans();
                // Saving under an existing name replaces that plan.
                saved.retain(|(existing, _)| *existing != name);
                saved.push((name, plan));
                // A short list stays a list of favourites, not a history.
                let excess = saved.len().saturating_sub(12);
                saved.drain(..excess);
                self.store_plans(&saved);
            }
            Event::DeletePlan(name) => {
                let mut saved = self.saved_plans();
                saved.retain(|(existing, _)| *existing != name);
                self.store_plans(&saved);
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
        match &mut self.window.session_window {
            Some(saved) => saved.taken = true,
            None => {
                self.window.session_window = Some(SavedWindow {
                    size: window.inner_size(),
                    position: window.outer_position().ok(),
                    maximized: window.is_maximized(),
                    fullscreen: window.fullscreen().is_some(),
                    pinned: self.window.pinned,
                    taken: true,
                });
            }
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
        if !self.window.pinned {
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
        let Some(saved) = self
            .window
            .session_window
            .as_mut()
            .filter(|saved| saved.taken)
        else {
            return;
        };
        saved.taken = false;
        let (size, position, maximized, fullscreen, pinned) = (
            saved.size,
            saved.position,
            saved.maximized,
            saved.fullscreen,
            saved.pinned,
        );
        let Some(window) = self.shell.window.clone() else {
            return;
        };
        // Unpin first: changing the stacking makes some compositors resend the small
        // window's geometry, which would undo a size set before it.
        if self.window.pinned != pinned {
            self.apply_pin(pinned);
        }
        window.set_fullscreen(fullscreen.then_some(winit::window::Fullscreen::Borderless(None)));
        // The old size first, so a compositor that ignores the maximize request
        // still gives the window back at its previous size.
        let applied = window.request_inner_size(size);
        self.resized(applied);
        if let Some(position) = position {
            window.set_outer_position(position);
        }
        if maximized {
            window.set_maximized(true);
        }
        self.window.restoring = (!maximized && !fullscreen)
            .then(|| (size, Instant::now() + std::time::Duration::from_secs(1)));
    }
    /// A late configure from the compositor can still carry the mini window's size
    /// right after a restore; ask again for the user's size then.
    pub(super) fn keep_restored(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        let Some((target, until)) = self.window.restoring else {
            return;
        };
        if Instant::now() > until {
            self.window.restoring = None;
            return;
        }
        if size != target
            && let Some(window) = self.shell.window.clone()
        {
            let applied = window.request_inner_size(target);
            self.resized(applied);
        }
    }
    /// Once the session is gone (closed, tab switched or board closed), the window is
    /// the user's again.
    pub(super) fn settle_session_window(&mut self) {
        if self.window.session_window.is_some() && self.workspace.board.session.is_none() {
            self.restore_window();
            self.window.session_window = None;
        }
    }
}
