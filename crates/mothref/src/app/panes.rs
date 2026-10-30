// SPDX-License-Identifier: GPL-3.0-or-later
//! Several main windows, each with its own tabs, window mode, pin and input.
//!
//! The application code works on "the current window" through `App`'s fields.
//! Every other main window keeps the same fields in a [`Pane`]; before an event
//! for another window is handled, its pane is swapped in. Settings, shortcuts,
//! caches and the GPU device are shared; each window has its own surface and
//! texture registry, because switching boards clears a registry.
use super::*;

macro_rules! pane {
    ($($field:ident: $ty:ty),+ $(,)?) => {
        /// The per-window state of a main window that is not current.
        pub(super) struct Pane {
            $($field: $ty,)+
            shell: shell::Parts,
        }
        impl Pane {
            /// Exchanges this window's state with the current window's.
            fn swap_with(&mut self, app: &mut App) {
                $(std::mem::swap(&mut self.$field, &mut app.$field);)+
                app.shell.swap_parts(&mut self.shell);
            }
        }
    };
}

pane! {
    workspace: workspace::Workspace,
    settings_window: settings::SettingsWindow,
    overlay: Option<overlay::Overlay>,
    floating: Vec<floating::ImageWindow>,
    applied_background: Option<(WindowId, bool, bool)>,
    pinned: bool,
    window_bar: bool,
    bottom: bool,
    resize_bounds: Option<Rect>,
    locked_position: Option<winit::dpi::PhysicalPosition<i32>>,
    target_pin: bool,
    picking_window: bool,
    moving_window: bool,
    pin: moth_platform::pin::Pin,
    library_screen: library::LibraryScreen,
    modal: modal::Modal,
    camera_motion: camera_motion::CameraMotion,
    camera_active: bool,
    items_active: bool,
    motion: motion::DragMotion,
    moving: Option<(Uuid, Rect)>,
    resizing: Option<resize::Resize>,
    drag_duplicate: bool,
    editing: Option<Editing>,
    drag: Option<([f32; 2], [f32; 2], bool)>,
    drag_anchor: Option<[f32; 2]>,
    drag_pointer: Option<(moth_platform::input::Kind, u64)>,
    input: InputState,
    window_closed: bool,
    session_window: Option<session_window::SavedWindow>,
    restoring: Option<(winit::dpi::PhysicalSize<u32>, Instant)>,
}

impl Pane {
    fn owns(&self, id: WindowId) -> bool {
        self.shell.window.as_ref().is_some_and(|w| w.id() == id)
            || self.overlay.as_ref().is_some_and(|o| o.window.id() == id)
            || self.floating.iter().any(|f| f.window.id() == id)
    }
    /// "MothRef (2) — board", so windows with the same board stay apart.
    fn title(&self) -> String {
        let window = self
            .shell
            .window
            .as_ref()
            .map(|w| w.title())
            .unwrap_or_default();
        format!("{window} — {}", self.workspace.board.title())
    }
    /// Closes the window, handing over its boards.
    fn into_documents(self) -> Vec<Document> {
        self.workspace.into_documents()
    }
}

/// A tab move between windows, queued during a frame.
pub(super) enum WindowOp {
    Detach(usize, Option<[f32; 2]>),
    Move(usize, WindowId),
    Gather,
}

/// The main window's attributes, shared by the first and every later window.
pub(super) fn attributes() -> winit::window::WindowAttributes {
    Window::default_attributes()
        .with_title(moth_ui::text(moth_ui::Text::Title))
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size(winit::dpi::LogicalSize::new(1440., 900.))
}

impl App {
    fn owns(&self, id: WindowId) -> bool {
        self.shell.window.as_ref().is_some_and(|w| w.id() == id)
            || self.overlay.as_ref().is_some_and(|o| o.window.id() == id)
            || self.floating.iter().any(|f| f.window.id() == id)
    }
    /// Makes the main window that owns `id` (or its overlay controller or
    /// floating views) current. False when no window owns it.
    pub(super) fn enter(&mut self, id: WindowId) -> bool {
        if self.owns(id) {
            return true;
        }
        let Some(index) = self.panes.iter().position(|p| p.owns(id)) else {
            return false;
        };
        let mut panes = std::mem::take(&mut self.panes);
        panes[index].swap_with(self);
        self.panes = panes;
        true
    }
    /// Runs `f` with every main window current in turn, for work that belongs to
    /// all of them (worker wake-ups, repaint requests, deadlines).
    pub(super) fn each_pane(&mut self, mut f: impl FnMut(&mut App)) {
        f(self);
        let mut panes = std::mem::take(&mut self.panes);
        for pane in &mut panes {
            pane.swap_with(self);
            f(self);
            pane.swap_with(self);
        }
        // Windows created meanwhile were pushed to the emptied list.
        panes.append(&mut self.panes);
        self.panes = panes;
    }
    /// Titles of the other main windows, for "Move to window".
    pub(super) fn window_titles(&self) -> Vec<String> {
        self.panes.iter().map(Pane::title).collect()
    }
    /// Drops closed windows. Returns false when the last window closed.
    pub(super) fn reap_windows(&mut self) -> bool {
        self.panes.retain(|pane| !pane.window_closed);
        if self.window_closed {
            let Some(mut next) = self.panes.pop() else {
                return false;
            };
            // The closed window's state moves into `next`, which is dropped.
            next.swap_with(self);
            drop(next);
            self.redraw();
        }
        true
    }

    /// Creates a window and makes it current; the previous one becomes a pane.
    fn open_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        board: Document,
        at: Option<winit::dpi::PhysicalPosition<f64>>,
    ) -> anyhow::Result<()> {
        let gpu = self
            .shell
            .gpu
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No renderer"))?;
        // Compositor integrations find windows by title, so every main window
        // gets its own: "MothRef", "MothRef (2)", …
        let base = moth_ui::text(moth_ui::Text::Title);
        let taken: Vec<String> = std::iter::once(&self.shell.window)
            .chain(self.panes.iter().map(|p| &p.shell.window))
            .filter_map(|w| w.as_ref().map(|w| w.title()))
            .collect();
        let title = (2..)
            .map(|n| format!("{base} ({n})"))
            .find(|t| !taken.contains(t))
            .unwrap_or_else(|| base.to_owned());
        let mut attributes = attributes().with_title(title);
        if let Some(at) = at {
            attributes = attributes.with_position(at);
        }
        let window = Arc::new(event_loop.create_window(attributes)?);
        let parts = gpu.window_parts(window.clone())?;
        let ctx = self.shell.new_context();
        let egui = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(2048),
        );
        let wake = self.shell.wake();
        let library = moth_platform::library::Worker::new(
            Arc::new(moth_platform::DesktopStorage::default()),
            wake.clone(),
        )?;
        let mut workspace = workspace::Workspace::new(board, wake.clone());
        workspace.library = false;
        let mut input = InputState::default();
        if let Err(error) = input.adapter.attach(
            window.clone(),
            event_loop.owned_display_handle(),
            wake.clone(),
        ) {
            tracing::warn!(%error, "native input startup failed");
        }
        let mut pane = Pane {
            workspace,
            settings_window: settings::SettingsWindow::default(),
            overlay: None,
            floating: Vec::new(),
            applied_background: None,
            pinned: false,
            window_bar: false,
            bottom: false,
            resize_bounds: None,
            locked_position: None,
            target_pin: false,
            picking_window: false,
            moving_window: false,
            pin: moth_platform::pin::Pin::new(wake),
            library_screen: library::LibraryScreen::new(library),
            modal: modal::Modal::default(),
            camera_motion: camera_motion::CameraMotion::default(),
            camera_active: false,
            items_active: false,
            motion: motion::DragMotion::default(),
            moving: None,
            resizing: None,
            drag_duplicate: false,
            editing: None,
            drag: None,
            drag_anchor: None,
            drag_pointer: None,
            input,
            window_closed: false,
            session_window: None,
            restoring: None,
            shell: shell::Parts {
                window: Some(window),
                ctx,
                egui: Some(egui),
                deadline: None,
                refinements: Default::default(),
                gpu: Some(parts),
            },
        };
        pane.swap_with(self);
        self.panes.push(pane);
        self.after_board_moved();
        Ok(())
    }

    /// The board came from another window: its textures live in that window.
    fn after_board_moved(&mut self) {
        self.reset_board_ui();
        self.adapt_gpu_budget();
        self.applied_background = None;
        if let Some(window) = &self.shell.window {
            let _ = moth_platform::Desktop.window_effects(window, false, false, false);
            window.focus_window();
        }
        self.redraw();
    }

    /// "Move to new window", or a tab dragged out of the tab strip.
    pub(super) fn tear_off(
        &mut self,
        event_loop: &ActiveEventLoop,
        slot: usize,
        at: Option<winit::dpi::PhysicalPosition<f64>>,
    ) {
        // A window's only tab is already a window of its own.
        let Some(board) = self.workspace.take(slot) else {
            return;
        };
        self.reset_board_ui();
        self.redraw();
        if let Err(error) = self.open_window(event_loop, board, at) {
            tracing::warn!(%error, "cannot open a board window");
            self.workspace.board.notices.message = error.to_string().into();
        }
    }

    /// Moves a tab of the current window into another window and focuses it.
    /// Moving a window's last tab closes that window.
    pub(super) fn move_tab(&mut self, slot: usize, window: usize) {
        if window >= self.panes.len() || self.workspace.confirm_close {
            return;
        }
        let board = self.workspace.take(slot);
        if board.is_some() {
            self.reset_board_ui();
            self.redraw();
        }
        let mut panes = std::mem::take(&mut self.panes);
        panes[window].swap_with(self);
        let boards = match board {
            Some(board) => vec![board],
            // The source held only this board: it is now `panes[window]`.
            None => panes.remove(window).into_documents(),
        };
        self.panes = panes;
        for board in boards {
            self.workspace.adopt(board);
        }
        self.after_board_moved();
    }

    /// Moves every board of every other window into the current one and closes
    /// those windows.
    pub(super) fn gather(&mut self) {
        for pane in std::mem::take(&mut self.panes) {
            for board in pane.into_documents() {
                self.workspace.adopt(board);
            }
        }
        self.after_board_moved();
    }

    /// Where a tab dragged outside the window was released: another window's
    /// tab strip moves it there, anywhere else opens a new window. Window
    /// positions are unknown on Wayland, so there it always opens a new window.
    pub(super) fn drop_tab(
        &mut self,
        event_loop: &ActiveEventLoop,
        slot: usize,
        position: [f32; 2],
    ) {
        let screen = self.shell.window.as_ref().and_then(|window| {
            let origin = window.inner_position().ok()?;
            let scale = window.scale_factor() * f64::from(self.shell.context().zoom_factor());
            Some(winit::dpi::PhysicalPosition::new(
                f64::from(origin.x) + f64::from(position[0]) * scale,
                f64::from(origin.y) + f64::from(position[1]) * scale,
            ))
        });
        if let Some(point) = screen {
            let strip = f64::from(moth_ui::tokens(self.shell.context()).space_header) * 2.;
            let target = self.panes.iter().position(|pane| {
                pane.shell.window.as_ref().is_some_and(|window| {
                    let (Ok(origin), size) = (window.inner_position(), window.inner_size()) else {
                        return false;
                    };
                    let scale = window.scale_factor();
                    point.x >= f64::from(origin.x)
                        && point.x < f64::from(origin.x) + f64::from(size.width)
                        && point.y >= f64::from(origin.y)
                        && point.y < f64::from(origin.y) + strip * scale
                })
            });
            if let Some(target) = target {
                self.move_tab(slot, target);
                return;
            }
        }
        self.tear_off(event_loop, slot, screen);
    }

    pub(super) fn queue_window_op(&mut self, op: WindowOp) {
        if let Some(window) = &self.shell.window {
            self.window_ops.push((window.id(), op));
        }
    }
    /// The main window shown at `index` in "Move to window".
    pub(super) fn pane_window(&self, index: usize) -> Option<WindowId> {
        self.panes.get(index)?.shell.window.as_ref().map(|w| w.id())
    }
    /// Runs tab moves requested during frames, each from its own window.
    pub(super) fn run_window_ops(&mut self, event_loop: &ActiveEventLoop) {
        for (window, op) in std::mem::take(&mut self.window_ops) {
            if !self.enter(window) {
                continue;
            }
            match op {
                WindowOp::Detach(slot, Some(position)) => self.drop_tab(event_loop, slot, position),
                WindowOp::Detach(slot, None) => self.tear_off(event_loop, slot, None),
                WindowOp::Move(slot, target) => {
                    let index = self.panes.iter().position(|pane| {
                        pane.shell.window.as_ref().is_some_and(|w| w.id() == target)
                    });
                    if let Some(index) = index {
                        self.move_tab(slot, index);
                    }
                }
                WindowOp::Gather => self.gather(),
            }
        }
    }
    /// After device loss the other windows' surfaces belong to the old device.
    pub(super) fn rebuild_window_surfaces(&mut self) {
        let Some(gpu) = &self.shell.gpu else {
            return;
        };
        for pane in &mut self.panes {
            pane.shell.gpu = pane
                .shell
                .window
                .clone()
                .and_then(|window| gpu.window_parts(window).ok());
            pane.workspace.board.reset_preload();
        }
    }

    /// Settings changes (theme, interface scale) reach every window.
    pub(super) fn restyle_windows(&self) {
        let tokens = self.settings.tokens();
        for pane in &self.panes {
            moth_ui::theme::apply(&pane.shell.ctx, &tokens);
            pane.shell.ctx.set_zoom_factor(self.settings.ui_scale);
            if let Some(window) = &pane.shell.window {
                window.request_redraw();
            }
        }
    }
    /// Main windows sharing one device split its video-memory limit.
    pub(super) fn window_count(&self) -> u64 {
        1 + self.panes.len() as u64
    }
}
