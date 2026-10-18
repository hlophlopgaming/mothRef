// SPDX-License-Identifier: GPL-3.0-or-later
//! Applies board and window actions through one path shared by UI and keyboard input.
use super::modal::Screen;
use super::{App, arrange, mirrored, rotated, transform_selection};
use moth_platform::Platform;
use moth_scene::{Command, Scene, Transform, Uuid};
use std::collections::HashSet;
use std::path::PathBuf;
use winit::{
    event::KeyEvent,
    event_loop::ActiveEventLoop,
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
};

/// Represents an application action independently of the UI or key that requested it.
pub(super) enum Intent {
    NewBoard,
    Open(PathBuf),
    OpenDialog,
    /// Checkpoint submission stays separate from opening the save-path editor.
    Save,
    SaveDialog,
    Close {
        window: bool,
    },
    CloseTab(usize),
    Discard,
    Undo,
    Redo,
    Commands(Vec<Command>),
    Arrange(arrange::Arrange, Vec<Uuid>),
    OriginalGeometry {
        ids: Vec<Uuid>,
        mode: moth_platform::export::Geometry,
    },
    FitAll,
    ScheduleFit,
    FitSelection,
    AddNote([f32; 2]),
    EditNote(Uuid),
    CommitNote,
    Export(moth_platform::export::Target),
    ActivateTab(usize),
    TogglePin,
    ToggleOverlay,
    Library,
    Paste,
    Zoom(f32),
    Pan(bool),
    Minimize,
    ToggleMaximize,
    DragWindow,
    Language(moth_ui::Locale),
    Preferences,
}
impl App {
    pub(super) fn apply(&mut self, intent: Intent, event_loop: &ActiveEventLoop) {
        match intent {
            Intent::Preferences => {
                self.shell.ctx.set_theme(if self.tools.dark {
                    egui::Theme::Dark
                } else {
                    egui::Theme::Light
                });
                self.shell.ctx.set_zoom_factor(self.tools.ui_scale);
                for tab in &mut self.workspace.tabs {
                    let values = std::collections::BTreeMap::from([(
                        "undo_limit".to_owned(),
                        self.tools.undo_limit.to_string(),
                    )]);
                    if tab
                        .document
                        .boards
                        .requests
                        .try_send(moth_platform::boards::Request::Preferences(values))
                        .is_ok()
                    {
                        tab.document.in_flight += 1;
                    } else {
                        tab.view.message = moth_ui::text(moth_ui::Text::StorageBusy).into();
                    }
                }
                self.persist(moth_platform::boards::Request::Preferences(
                    self.tools
                        .preferences()
                        .into_iter()
                        .map(|(k, v)| (k.to_owned(), v))
                        .collect(),
                ));
            }
            Intent::ScheduleFit => {
                self.workspace.board.view.needs_fit = true;
            }
            Intent::FitSelection => {
                let selected: Vec<_> = self
                    .workspace
                    .board
                    .view
                    .selection
                    .iter()
                    .filter_map(|id| self.workspace.board.document.scene.get(*id))
                    .collect();
                if !selected.is_empty() {
                    let min = selected.iter().fold([f32::INFINITY; 2], |a, i| {
                        let e = i.rect.rotated_envelope(i.transform.rotation);
                        [a[0].min(e.lower()[0]), a[1].min(e.lower()[1])]
                    });
                    let max = selected.iter().fold([f32::NEG_INFINITY; 2], |a, i| {
                        [
                            a[0].max(i.rect.rotated_envelope(i.transform.rotation).upper()[0]),
                            a[1].max(i.rect.rotated_envelope(i.transform.rotation).upper()[1]),
                        ]
                    });
                    self.workspace.board.view.camera.zoom =
                        ((self.workspace.board.view.viewport.width() - 48.) / (max[0] - min[0]))
                            .min(
                                (self.workspace.board.view.viewport.height() - 48.)
                                    / (max[1] - min[1]),
                            )
                            .clamp(0.01, 64.);
                    self.workspace.board.view.camera.origin = [
                        (min[0] + max[0]) / 2.
                            - self.workspace.board.view.viewport.center().x
                                / self.workspace.board.view.camera.zoom,
                        (min[1] + max[1]) / 2.
                            - self.workspace.board.view.viewport.center().y
                                / self.workspace.board.view.camera.zoom,
                    ];
                    self.shell.redraw();
                }
            }
            Intent::Commands(commands) => {
                self.persist_batch(commands.into_iter().map(|c| (c, None)).collect());
            }
            Intent::Arrange(mode, ids) => {
                let mut items: Vec<_> = ids
                    .iter()
                    .filter_map(|id| self.workspace.board.document.scene.get(*id))
                    .collect();
                items.sort_unstable_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                let target_width = self.workspace.board.view.viewport.width()
                    / self.workspace.board.view.camera.zoom;
                let commands = arrange::arrange(&items, mode, target_width, self.tools.gap);
                if !commands.is_empty() {
                    self.persist_batch(commands.into_iter().map(|c| (c, None)).collect());
                }
            }
            Intent::OriginalGeometry { ids, mode } => {
                self.persist(moth_platform::boards::Request::OriginalGeometry { ids, mode });
            }
            Intent::Export(target) => {
                self.persist(moth_platform::boards::Request::Export(target));
            }
            Intent::Paste => {
                if !self.workspace.board.document.discarding {
                    self.paste();
                }
            }
            Intent::FitAll => {
                self.fit();
            }
            Intent::Zoom(factor) => {
                self.workspace.board.view.camera.zoom_at(
                    [
                        self.workspace.board.view.viewport.center().x,
                        self.workspace.board.view.viewport.center().y,
                    ],
                    factor,
                );
                self.shell.redraw();
            }
            Intent::ToggleOverlay => {
                self.toggle_overlay(event_loop);
            }
            Intent::TogglePin => {
                let Some(window) = self.shell.window.clone() else {
                    return;
                };

                self.pinned = !self.pinned;
                if let Err(error) = moth_platform::Desktop.window_effects(
                    &window,
                    self.overlay.is_some(),
                    self.pinned,
                    false,
                ) {
                    self.workspace.board.view.message = error;
                }
            }
            Intent::Library => {
                self.workspace.screen = Screen::Library;
                self.library.refresh = true;
            }
            Intent::NewBoard => {
                self.new_board();
            }
            Intent::ActivateTab(index) => {
                self.activate(index);
            }
            Intent::CloseTab(index) => {
                self.activate(index);
                self.request_close(false);
            }
            Intent::Close {
                window: closing_window,
            } => {
                self.request_close(closing_window);
            }
            Intent::Minimize => {
                let Some(window) = self.shell.window.clone() else {
                    return;
                };

                window.set_minimized(true);
            }
            Intent::ToggleMaximize => {
                let Some(window) = self.shell.window.clone() else {
                    return;
                };

                window.set_maximized(!window.is_maximized());
            }
            Intent::DragWindow => {
                let Some(window) = self.shell.window.clone() else {
                    return;
                };

                let _ = window.drag_window();
            }
            Intent::Language(language) => {
                moth_ui::set_locale(language);
                self.persist(moth_platform::boards::Request::SaveSettings(
                    language.code().to_owned(),
                ));
            }
            Intent::OpenDialog => {
                self.open_files();
            }
            Intent::SaveDialog => {
                self.modal.open_path();
            }
            Intent::Open(path) => {
                if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("moth"))
                {
                    self.open_in_tab(path);
                } else {
                    if self.workspace.screen == Screen::Library {
                        self.new_board();
                    }
                    self.import(path);
                }
            }
            Intent::Save => {
                self.persist(moth_platform::boards::Request::Save(
                    std::path::PathBuf::from(&self.workspace.board.document.path),
                ));
            }
            Intent::Undo => {
                self.persist(moth_platform::boards::Request::Undo);
            }
            Intent::Redo => {
                self.persist(moth_platform::boards::Request::Redo);
            }
            Intent::Discard => {
                self.discard();
            }
            Intent::AddNote(world) => {
                self.add_note(world);
            }
            Intent::EditNote(id) => {
                self.edit_note(id);
            }
            Intent::CommitNote => {
                self.finish_editing();
            }
            Intent::Pan(pan) => {
                self.workspace.board.view.pan = pan;
                self.shell.redraw();
            }
        }
    }
    /// Keeps shortcut guards and their redraw timing alongside application actions.
    pub(super) fn keyboard(&mut self, event: &KeyEvent, event_loop: &ActiveEventLoop) {
        if self.modal.path().is_some()
            || self.modal.confirming()
            || self.workspace.board.document.arrange_prompt.is_some()
        {
            return;
        }
        if self.command
            && !self.shell.ctx.egui_wants_keyboard_input()
            && event.physical_key == PhysicalKey::Code(KeyCode::KeyN)
        {
            self.apply(Intent::NewBoard, event_loop);
        } else if self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyO) {
            self.apply(Intent::OpenDialog, event_loop);
        } else if self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyL) {
            self.apply(Intent::Library, event_loop);
            self.shell.redraw();
        } else if self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyW) {
            self.apply(Intent::Close { window: false }, event_loop);
        } else if self.command && event.logical_key == Key::Named(NamedKey::Tab) {
            if !self.workspace.tabs.is_empty() {
                self.apply(Intent::ActivateTab(1), event_loop);
            }
        } else if (self.overlay.is_some() && event.logical_key == Key::Named(NamedKey::Escape))
            || (self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyT))
        {
            self.apply(Intent::ToggleOverlay, event_loop);
        } else if self.command
            && !self.shell.ctx.egui_wants_keyboard_input()
            && (event.physical_key == PhysicalKey::Code(KeyCode::KeyV)
                || matches!(event.logical_key,Key::Character(ref c) if c.eq_ignore_ascii_case("v")))
        {
            self.apply(Intent::Paste, event_loop);
        } else if self.command
            && event.physical_key == PhysicalKey::Code(KeyCode::KeyS)
            && self.workspace.board.document.idle()
        {
            if self.workspace.board.document.path.is_empty() {
                self.apply(Intent::SaveDialog, event_loop);
                self.shell.redraw();
            } else {
                self.apply(Intent::Save, event_loop);
            }
        } else if self.command
            && !self.shell.ctx.egui_wants_keyboard_input()
            && event.physical_key == PhysicalKey::Code(KeyCode::KeyZ)
            && self.workspace.board.document.in_flight == 0
        {
            self.apply(Intent::Undo, event_loop);
        } else if self.command
            && !self.shell.ctx.egui_wants_keyboard_input()
            && event.physical_key == PhysicalKey::Code(KeyCode::KeyY)
            && self.workspace.board.document.in_flight == 0
        {
            self.apply(Intent::Redo, event_loop);
        } else if !self.shell.ctx.egui_wants_keyboard_input()
            && !self.command
            && self.workspace.board.document.idle()
            && matches!(
                event.logical_key,
                Key::Named(
                    NamedKey::ArrowLeft
                        | NamedKey::ArrowRight
                        | NamedKey::ArrowUp
                        | NamedKey::ArrowDown
                )
            )
        {
            let distance = if self.shell.ctx.input(|i| i.modifiers.shift) {
                10.
            } else {
                1.
            };
            let delta = match event.logical_key {
                Key::Named(NamedKey::ArrowLeft) => [-distance, 0.],
                Key::Named(NamedKey::ArrowRight) => [distance, 0.],
                Key::Named(NamedKey::ArrowUp) => [0., -distance],
                _ => [0., distance],
            };
            let items: Vec<_> = self
                .workspace
                .board
                .view
                .selection
                .iter()
                .filter_map(|id| self.workspace.board.document.scene.get(*id))
                .collect();
            let commands = arrange::translate(
                &items,
                delta,
                false,
                self.workspace.board.document.scene.top(),
            );
            self.apply(Intent::Commands(commands), event_loop);
        } else if let Some(intent) = self.transform_keys(event) {
            self.apply(intent, event_loop);
        } else if !self.shell.ctx.egui_wants_keyboard_input()
            && event.logical_key == Key::Character("v".into())
        {
            self.apply(Intent::Pan(false), event_loop);
        } else if !self.shell.ctx.egui_wants_keyboard_input()
            && event.logical_key == Key::Character("h".into())
        {
            self.apply(Intent::Pan(true), event_loop);
        } else if !self.shell.ctx.egui_wants_keyboard_input()
            && event.logical_key == Key::Named(NamedKey::Home)
        {
            self.apply(Intent::FitAll, event_loop);
        }
    }
    /// Rotation, mirroring and reset keys; they act on the selection and need an idle board.
    fn transform_keys(&self, event: &KeyEvent) -> Option<Intent> {
        if self.command
            || self.shell.ctx.egui_wants_keyboard_input()
            || self.workspace.board.view.selection.is_empty()
            || !self.workspace.board.document.idle()
        {
            return None;
        }
        transform_key(
            event.physical_key,
            self.shell.ctx.input(|input| input.modifiers.shift),
            &self.workspace.board.document.scene,
            &self.workspace.board.view.selection,
        )
    }
}

/// Key mapping stays pure so UI and shortcut commands can be compared without a window.
fn transform_key(
    key: PhysicalKey,
    shift: bool,
    scene: &Scene,
    selection: &HashSet<Uuid>,
) -> Option<Intent> {
    let change = |f: fn(Transform) -> Transform| transform_selection(scene, selection, f);
    Some(match (key, shift) {
        (PhysicalKey::Code(KeyCode::KeyR), false) => Intent::Commands(change(|t| rotated(t, 1.))),
        (PhysicalKey::Code(KeyCode::KeyR), true) => Intent::Commands(change(|t| rotated(t, -1.))),
        (PhysicalKey::Code(KeyCode::KeyH), true) => Intent::Commands(change(|t| mirrored(t, 0))),
        (PhysicalKey::Code(KeyCode::KeyV), true) => Intent::Commands(change(|t| mirrored(t, 1))),
        (PhysicalKey::Code(KeyCode::Digit0), true) => Intent::OriginalGeometry {
            ids: selection.iter().copied().collect(),
            mode: moth_platform::export::Geometry::All,
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use moth_scene::{Item, Rect};

    fn fixture() -> (Scene, HashSet<Uuid>) {
        let mut scene = Scene::default();
        let mut selection = HashSet::new();
        for index in 0..3 {
            let mut item = Item::text(
                "note",
                Rect {
                    x: index as f32 * 40.,
                    y: 0.,
                    width: 20.,
                    height: 30.,
                },
                scene.top(),
            );
            item.transform = Transform {
                rotation: 0.25,
                opacity: 0.4,
                grayscale: true,
                content_scale: 2.,
                ..Transform::default()
            };
            if index < 2 {
                selection.insert(item.id);
            }
            scene.apply(Command::Insert(item)).unwrap();
        }
        (scene, selection)
    }

    #[test]
    fn transform_shortcuts_produce_the_same_commands_as_the_selection_menu() {
        let (scene, selection) = fixture();
        type Change = fn(Transform) -> Transform;
        let cases: [(KeyCode, bool, Change); 4] = [
            (KeyCode::KeyR, false, |t| rotated(t, 1.)),
            (KeyCode::KeyR, true, |t| rotated(t, -1.)),
            (KeyCode::KeyH, true, |t| mirrored(t, 0)),
            (KeyCode::KeyV, true, |t| mirrored(t, 1)),
        ];
        for (key, shift, change) in cases {
            let Some(Intent::Commands(commands)) =
                transform_key(PhysicalKey::Code(key), shift, &scene, &selection)
            else {
                panic!("transform shortcut must produce a command batch");
            };
            let expected = transform_selection(&scene, &selection, change);
            for (actual, expected) in commands.iter().zip(&expected) {
                let (
                    Command::SetTransform { id, transform },
                    Command::SetTransform {
                        id: expected_id,
                        transform: expected_transform,
                    },
                ) = (actual, expected)
                else {
                    panic!("unexpected command")
                };
                assert_eq!(id, expected_id);
                assert_eq!(transform, expected_transform);
            }
            assert_eq!(commands.len(), selection.len());
            for command in commands {
                let Command::SetTransform { id, transform } = command else {
                    panic!("unexpected command")
                };
                assert!(selection.contains(&id));
                assert_eq!(transform.opacity, 0.4);
                assert!(transform.grayscale);
                assert_eq!(transform.content_scale, 2.);
            }
        }
    }

    #[test]
    fn reset_shortcut_requests_original_geometry_for_the_entire_selection() {
        let (scene, selection) = fixture();
        let Some(Intent::OriginalGeometry { ids, mode }) =
            transform_key(PhysicalKey::Code(KeyCode::Digit0), true, &scene, &selection)
        else {
            panic!("reset must use the same original-geometry request as the menu")
        };
        assert!(matches!(mode, moth_platform::export::Geometry::All));
        assert_eq!(ids.into_iter().collect::<HashSet<_>>(), selection);
    }

    #[test]
    fn unmodified_view_keys_and_unrelated_keys_are_left_for_their_existing_handlers() {
        let (scene, selection) = fixture();
        for key in [
            KeyCode::KeyH,
            KeyCode::KeyV,
            KeyCode::Digit0,
            KeyCode::ArrowLeft,
        ] {
            assert!(transform_key(PhysicalKey::Code(key), false, &scene, &selection).is_none());
        }
    }
}
