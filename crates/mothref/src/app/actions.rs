// SPDX-License-Identifier: GPL-3.0-or-later
//! All registered actions share availability and execution, from keys or command search.
use super::*;
use bindings::Action;
impl App {
    pub(super) fn action_enabled(&self, action: Action) -> bool {
        use Action::*;
        match action {
            ContinueConversion | SaveConversion => {
                return self.workspace.board.conversion_prompt && self.workspace.board.idle();
            }
            SaveAndClose => {
                return self.workspace.confirm_close
                    && self.workspace.board.in_flight == 0
                    && self.workspace.board.pasting == 0
                    && self.workspace.board.pending.is_empty()
                    && self.workspace.board.waiting.is_empty();
            }
            Discard | KeepOpen => return self.workspace.confirm_close,
            CancelDialog => return self.modal.dialog.is_some(),
            SavePath => {
                return self.modal.dialog.is_some() && !self.modal.dialog_path.trim().is_empty();
            }
            DismissRecovery | RestoreRecovery => {
                return !self.workspace.recovery_paths().is_empty() && self.workspace.board.idle();
            }
            CancelArrange => return self.workspace.board.arrange_prompt.is_some(),
            AddedHorizontal | AddedVertical | AddedGrid | AddedOptimal | UndoAdded
            | DismissAdded => {
                return self.workspace.board.added.is_some()
                    && self.workspace.board.idle()
                    && !self.tools.locked;
            }
            _ => {}
        }
        if action == ClickThrough {
            return self.overlay.is_some()
                && self
                    .shell
                    .window
                    .as_ref()
                    .is_some_and(|w| moth_platform::Desktop.window_capabilities(w).click_through);
        }
        if self.editing.is_some() {
            return false;
        }
        if matches!(
            action,
            Palette
                | BindingSettings
                | Shortcuts
                | Settings
                | CloseWindow
                | New
                | Open
                | Images
                | Library
                | CloseTab
                | NextTab
                | Overlay
                | Minimize
                | Maximize
                | Fullscreen
        ) {
            return true;
        }
        if action == WindowBar {
            return self.compact();
        }
        if action == ShellIntegration {
            return matches!(
                moth_platform::Desktop.shell_integration(),
                moth_platform::ShellIntegration::Missing | moth_platform::ShellIntegration::Pending
            );
        }
        if action == ClearDiskCache {
            return !self.tools.disk_cache_busy && !self.tools.clear_disk_cache;
        }
        if action == MoveWindow {
            return !self.window_position_locked();
        }
        if action == LockWindow {
            return self
                .shell
                .window
                .as_ref()
                .is_some_and(|w| w.outer_position().is_ok());
        }
        if action == OverlaySelection {
            return !self.workspace.library
                && !self.workspace.board.loading
                && self.workspace.board.selection.iter().any(|id| {
                    self.workspace
                        .board
                        .scene
                        .get(*id)
                        .is_some_and(|item| item.blob().is_some())
                });
        }
        if matches!(action, ResizeSelection | ResizeAll) {
            return !self.workspace.library
                && !self.workspace.board.loading
                && !self.workspace.board.scene.is_empty()
                && (action == ResizeAll || !self.workspace.board.selection.is_empty())
                && self
                    .shell
                    .window
                    .as_ref()
                    .is_some_and(|w| w.fullscreen().is_none() && !w.is_maximized());
        }
        if matches!(action, OpacityUp | OpacityDown) {
            return !self.workspace.library;
        }
        if matches!(action, Pin | Bottom) {
            return self.overlay.is_none()
                && self.shell.window.as_ref().is_some_and(|w| {
                    let caps = moth_platform::Desktop.window_capabilities(w);
                    if action == Pin {
                        caps.on_top || caps.above_window
                    } else {
                        caps.bottom
                    }
                });
        }
        if self.workspace.library {
            return false;
        }
        if matches!(
            action,
            PanLeft
                | PanRight
                | PanUp
                | PanDown
                | Fit
                | Focus
                | Previous
                | Next
                | ZoomIn
                | ZoomOut
                | ZoomReset
                | ResetCamera
                | Select
                | ToggleView
        ) {
            return true;
        }
        if action == Deselect {
            return !self.workspace.board.selection.is_empty();
        }
        if matches!(action, Lock | Grayscale | Save) {
            return self.workspace.board.idle();
        }
        if matches!(
            action,
            ExportScene | ExportSelection | ExportOriginal | ExportOriginals | ExportZip
        ) {
            if self.tools.path.is_empty() || (self.tools.crop && !self.tools.region.valid()) {
                return false;
            }
            if matches!(action, ExportScene | ExportOriginals | ExportZip) {
                return self.workspace.board.idle() && !self.workspace.board.scene.is_empty();
            }
        }
        if action == ExportSelection {
            return self.workspace.board.idle() && !self.workspace.board.selection.is_empty();
        }
        if matches!(action, ExportOriginal | CopyImage | OpenOriginal) {
            return self.workspace.board.idle()
                && self.workspace.board.selection.len() == 1
                && self.workspace.board.selection.iter().any(|id| {
                    self.workspace
                        .board
                        .scene
                        .get(*id)
                        .is_some_and(|i| i.blob().is_some())
                });
        }
        if matches!(action, Export | SampleColor | CoordinatePicker) {
            return self.workspace.board.idle() && !self.workspace.board.scene.is_empty();
        }
        if matches!(
            action,
            SamplingDefault | SamplingSmooth | SamplingNearest | SamplingAutomatic
        ) {
            return self.workspace.board.idle()
                && !self.tools.locked
                && !self.workspace.board.selection.is_empty()
                && self.workspace.board.selection.iter().all(|id| {
                    self.workspace
                        .board
                        .scene
                        .get(*id)
                        .is_some_and(|item| item.blob().is_some())
                });
        }
        if action == EditNote {
            return self.workspace.board.idle()
                && !self.tools.locked
                && self.workspace.board.selection.len() == 1
                && self.workspace.board.selection.iter().any(|id| {
                    self.workspace
                        .board
                        .scene
                        .get(*id)
                        .is_some_and(|i| i.note().is_some())
                });
        }
        if matches!(action, EditItem | ApplyItem | CancelItem)
            && self.workspace.board.selection.len() != 1
        {
            return false;
        }
        if action == ApplyItem
            && !self.tools.draft.is_some_and(|(id, r, t)| {
                self.workspace.board.selection.contains(&id) && r.valid() && t.valid()
            })
        {
            return false;
        }
        if !self.workspace.board.idle() || self.tools.locked {
            return false;
        }
        if matches!(action, Paste | Undo | Redo | Note) {
            return true;
        }
        if matches!(
            action,
            Horizontal
                | Vertical
                | Grid
                | Optimal
                | ArrangeName
                | ArrangeTime
                | PackFit
                | NormalizeWidth
                | NormalizeHeight
                | NormalizeSize
                | NormalizeArea
                | AlignLeft
                | AlignRight
                | AlignTop
                | AlignBottom
                | Stack
                | DistributeHorizontal
                | DistributeVertical
                | ArrangeOptions
                | ScaleApply
        ) {
            return !self.workspace.board.scene.is_empty();
        }
        !self.workspace.board.selection.is_empty()
    }
    pub(super) fn run_action(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        use Action::*;
        if !self.action_enabled(action) {
            return;
        }
        if matches!(
            action,
            AddedHorizontal | AddedVertical | AddedGrid | AddedOptimal
        ) {
            if let Some(added) = &self.workspace.board.added {
                let ids = added.ids.clone();
                let mode = match action {
                    AddedHorizontal => arrange::Arrange::Horizontal,
                    AddedVertical => arrange::Arrange::Vertical,
                    AddedGrid => arrange::Arrange::Grid,
                    _ => arrange::Arrange::Optimal,
                };
                self.apply(Intent::Arrange(mode, ids), event_loop);
            }
            return;
        }
        if matches!(action, Horizontal | Vertical | Grid | Optimal)
            && let Some(ids) = self.workspace.board.arrange_prompt.clone()
        {
            let mode = match action {
                Horizontal => arrange::Arrange::Horizontal,
                Vertical => arrange::Arrange::Vertical,
                Grid => arrange::Arrange::Grid,
                _ => arrange::Arrange::Optimal,
            };
            self.apply(Intent::Arrange(mode, ids), event_loop);
            self.apply(Intent::DismissArrangePrompt, event_loop);
            return;
        }
        let mut items: Vec<_> = self
            .workspace
            .board
            .selection
            .iter()
            .filter_map(|id| self.workspace.board.scene.get(*id))
            .collect();
        items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
        let intent = match action {
            Palette => {
                self.cancel_drag();
                self.shortcuts.open_palette();
                return;
            }
            Shortcuts => {
                self.cancel_drag();
                self.shortcuts.open_cheatsheet();
                return;
            }
            BindingSettings => {
                self.cancel_drag();
                self.shortcuts.open_settings();
                return;
            }
            ContinueConversion => {
                self.workspace.board.conversion_prompt = false;
                return;
            }
            SaveConversion => {
                self.workspace.board.conversion_prompt = false;
                Intent::Save
            }
            SaveAndClose => Intent::SaveAndClose,
            Discard => Intent::Discard,
            KeepOpen => Intent::KeepOpen,
            CancelDialog => Intent::CancelDialog,
            DismissRecovery => Intent::DismissRecovery,
            RestoreRecovery => Intent::RestoreRecovery,
            DismissAdded => {
                self.workspace.finish_added();
                return;
            }
            UndoAdded => Intent::Undo,
            AddedHorizontal | AddedVertical | AddedGrid | AddedOptimal => return,
            CancelArrange => Intent::DismissArrangePrompt,
            SavePath => Intent::SaveAs(std::path::PathBuf::from(&self.modal.dialog_path)),
            Settings => {
                self.cancel_drag();
                self.settings_window.open = !self.settings_window.open;
                return;
            }
            CloseWindow => Intent::CloseWindow,
            ClearDiskCache => {
                self.tools.clear_disk_cache = true;
                return;
            }
            WindowBar => {
                self.window_bar = !self.window_bar;
                return;
            }
            ShellIntegration => {
                moth_platform::Desktop.install_shell_integration();
                self.workspace.board.message = moth_ui::Text::ShellIntegrationInstalling.into();
                return;
            }
            EditNote => {
                let [item] = items.as_slice() else {
                    return;
                };
                Intent::EditNote(item.id)
            }
            EditItem => {
                self.tools.inspector = true;
                return;
            }
            ArrangeOptions => {
                self.tools.arrange_open = true;
                return;
            }
            Export => {
                self.workspace.board.export_message = None;
                self.tools.export_open = true;
                return;
            }
            SampleColor => {
                self.workspace.board.export_message = None;
                self.tools.picking = true;
                self.tools.coordinates = false;
                self.tools.picker_open = true;
                return;
            }
            CoordinatePicker => {
                self.workspace.board.export_message = None;
                self.tools.coordinates = true;
                self.tools.picking = false;
                self.tools.picker_open = true;
                return;
            }
            SamplingDefault | SamplingSmooth | SamplingNearest | SamplingAutomatic => {
                let sampling = match action {
                    SamplingSmooth => moth_scene::Sampling::Smooth,
                    SamplingNearest => moth_scene::Sampling::Nearest,
                    SamplingAutomatic => moth_scene::Sampling::Automatic,
                    _ => moth_scene::Sampling::Default,
                };
                Intent::Commands(
                    items
                        .iter()
                        .filter(|item| item.transform.sampling != sampling)
                        .map(|item| Command::SetTransform {
                            id: item.id,
                            transform: Transform {
                                sampling,
                                ..item.transform
                            },
                        })
                        .collect(),
                )
            }
            ApplyItem => {
                let Some((id, mut rect, transform)) = self.tools.draft else {
                    return;
                };
                let Some(item) = self.workspace.board.scene.get(id) else {
                    return;
                };
                if let Some(text) = item.note() {
                    let [width, height] = canvas::measure_note_frame(
                        self.shell.context(),
                        text,
                        transform.note_font_size.max(0.001),
                        transform.note_wrap.then_some(rect.width),
                    );
                    rect.width = width;
                    rect.height = height;
                }
                if !rect.valid() || !transform.valid() {
                    return;
                }
                let mut commands = Vec::new();
                if item.rect != rect {
                    commands.push(Command::SetRect { id, rect });
                }
                if item.transform != transform {
                    commands.push(Command::SetTransform { id, transform });
                }
                Intent::Commands(commands)
            }
            CancelItem => {
                self.tools.draft = None;
                self.tools.observed = None;
                return;
            }
            ScaleApply => {
                if items.is_empty() {
                    items = self.workspace.board.scene.items().collect();
                    items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                }
                Intent::Commands(arrange::scale(&items, self.tools.factor))
            }
            ResetRotation | ResetFlip => Intent::Commands(transform_selection(
                &self.workspace.board.scene,
                &self.workspace.board.selection,
                |t| {
                    let mut t = t;
                    if action == ResetRotation {
                        t.rotation = 0.;
                    } else {
                        t.flip = [false; 2];
                    }
                    t
                },
            )),
            ResetScale | NormalizeScale => Intent::OriginalGeometry(
                items
                    .iter()
                    .filter(|i| i.blob().is_some())
                    .map(|i| i.id)
                    .collect(),
                if action == ResetScale {
                    moth_platform::export::Geometry::Scale
                } else {
                    moth_platform::export::Geometry::Normalize
                },
            ),
            OpenOriginal | CopyImage | ExportScene | ExportSelection | ExportOriginal
            | ExportOriginals | ExportZip => {
                use moth_platform::export::Target;
                let path = std::path::PathBuf::from(&self.tools.path);
                let target = match action {
                    OpenOriginal => {
                        self.tools.viewer_open = true;
                        let [item] = items.as_slice() else {
                            return;
                        };
                        Target::Viewer(item.id)
                    }
                    CopyImage => {
                        let [item] = items.as_slice() else {
                            return;
                        };
                        Target::Clipboard(item.id)
                    }
                    ExportOriginal => {
                        let [item] = items.as_slice() else {
                            return;
                        };
                        Target::Original { path, id: item.id }
                    }
                    ExportOriginals => Target::Originals(path),
                    ExportZip => Target::Archive {
                        path,
                        ids: items.iter().map(|i| i.id).collect(),
                    },
                    _ => Target::Scene {
                        path,
                        ids: if action == ExportSelection {
                            items.iter().map(|i| i.id).collect()
                        } else {
                            Vec::new()
                        },
                        region: self.tools.crop.then_some(self.tools.region),
                        side: self.tools.side,
                    },
                };
                Intent::Export(target)
            }
            New => Intent::New,
            Open => Intent::Open,
            Images => Intent::Images,
            Save => Intent::Save,
            Library => Intent::Home,
            CloseTab => Intent::CloseTab(0),
            NextTab => {
                if !self.workspace.tabs.is_empty() {
                    self.activate(1);
                }
                return;
            }
            Paste => Intent::Paste,
            Undo => Intent::Undo,
            Redo => Intent::Redo,
            ToggleView => {
                self.workspace.board.appearance.view = !self.workspace.board.appearance.view;
                self.workspace.board.appearance.over = false;
                self.remember_appearance();
                return;
            }
            Overlay => Intent::Overlay,
            Pin => Intent::Pin,
            ClickThrough => {
                if let Some(overlay) = &mut self.overlay {
                    overlay.click_through = !overlay.click_through;
                    overlay.window.request_redraw();
                }
                return;
            }
            OverlaySelection => {
                self.float_selection(event_loop);
                return;
            }
            Bottom => {
                let enabled = !self.bottom;
                self.apply_pin(false);
                self.bottom = enabled;
                if let Some(window) = &self.shell.window {
                    moth_platform::Desktop.window_bottom(window, enabled);
                }
                return;
            }
            LockWindow => {
                self.locked_position = if self.locked_position.is_some() {
                    None
                } else {
                    self.shell
                        .window
                        .as_ref()
                        .and_then(|w| w.outer_position().ok())
                };
                self.cancel_window_mode();
                return;
            }
            ResizeSelection | ResizeAll => {
                self.resize_to_items(action == ResizeSelection);
                return;
            }
            OpacityUp | OpacityDown => {
                let appearance = &mut self.workspace.board.appearance;
                appearance.window =
                    window_mode::adjust_opacity(appearance.window, action == OpacityUp);
                if let Some(overlay) = &mut self.overlay {
                    overlay.appearance = *appearance;
                }
                self.remember_appearance();
                return;
            }
            MoveWindow => {
                self.cancel_drag();
                let moving = !self.moving_window;
                self.cancel_window_mode();
                self.moving_window = moving;
                return;
            }
            Fullscreen => Intent::Fullscreen,
            Minimize => Intent::Minimize,
            Maximize => Intent::Maximize,
            Select => return,
            Deselect => {
                self.workspace.board.selection.clear();
                return;
            }
            Fit => Intent::Fit,
            Focus => Intent::FitSelection,
            Previous => Intent::Focus(-1),
            Next => Intent::Focus(1),
            ZoomIn => Intent::Zoom(1. + self.settings.zoom_increment / 100.),
            ZoomOut => Intent::Zoom(1. / (1. + self.settings.zoom_increment / 100.)),
            ZoomReset => Intent::ZoomTo(1.),
            PanLeft => Intent::Pan([-self.settings.keyboard_pan_step, 0.]),
            PanRight => Intent::Pan([self.settings.keyboard_pan_step, 0.]),
            PanUp => Intent::Pan([0., -self.settings.keyboard_pan_step]),
            PanDown => Intent::Pan([0., self.settings.keyboard_pan_step]),
            ResetCamera => Intent::ResetCamera,
            Note => Intent::AddNote(self.workspace.board.view.camera.world([
                self.workspace.board.view.viewport.center().x,
                self.workspace.board.view.viewport.center().y,
            ])),
            Delete => Intent::Commands(items.iter().map(|i| Command::Remove(i.id)).collect()),
            Duplicate => Intent::Commands(arrange::translate(
                &items,
                [24., 24.],
                true,
                self.workspace.board.scene.top(),
            )),
            RotateCw | RotateCcw | FlipHorizontal | FlipVertical => {
                Intent::Commands(transform_selection(
                    &self.workspace.board.scene,
                    &self.workspace.board.selection,
                    |t| match action {
                        RotateCw => rotated(t, 1.),
                        RotateCcw => rotated(t, -1.),
                        FlipHorizontal => mirrored(t, 0),
                        _ => mirrored(t, 1),
                    },
                ))
            }
            ResetTransform | ResetCrop => Intent::OriginalGeometry(
                items.iter().map(|i| i.id).collect(),
                if action == ResetCrop {
                    moth_platform::export::Geometry::Crop
                } else {
                    moth_platform::export::Geometry::All
                },
            ),
            NudgeLeft | NudgeRight | NudgeUp | NudgeDown | NudgeLeftFast | NudgeRightFast
            | NudgeUpFast | NudgeDownFast => {
                let delta = match action {
                    NudgeLeft => [-1., 0.],
                    NudgeRight => [1., 0.],
                    NudgeUp => [0., -1.],
                    NudgeDown => [0., 1.],
                    NudgeLeftFast => [-10., 0.],
                    NudgeRightFast => [10., 0.],
                    NudgeUpFast => [0., -10.],
                    _ => [0., 10.],
                };
                Intent::Commands(arrange::translate(
                    &items,
                    delta,
                    false,
                    self.workspace.board.scene.top(),
                ))
            }
            BringFront | SendBack | BringForward | SendBackward => {
                Intent::Commands(arrange::reorder(
                    &self.workspace.board.scene,
                    &self.workspace.board.selection,
                    matches!(action, BringFront | BringForward),
                    matches!(action, BringFront | SendBack),
                ))
            }
            Horizontal | Vertical | Grid | Optimal | ArrangeName | ArrangeTime | PackFit
            | NormalizeWidth | NormalizeHeight | NormalizeSize | NormalizeArea | AlignLeft
            | AlignRight | AlignTop | AlignBottom | Stack | DistributeHorizontal
            | DistributeVertical => {
                if items.is_empty() || action == PackFit {
                    items = self.workspace.board.scene.items().collect();
                    items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                }
                let commands = match action {
                    NormalizeWidth | NormalizeHeight | NormalizeSize | NormalizeArea => {
                        arrange::normalize(
                            &items,
                            match action {
                                NormalizeWidth => arrange::Normalize::Width,
                                NormalizeHeight => arrange::Normalize::Height,
                                NormalizeSize => arrange::Normalize::Size,
                                _ => arrange::Normalize::Area,
                            },
                        )
                    }
                    AlignLeft | AlignRight | AlignTop | AlignBottom | Stack
                    | DistributeHorizontal | DistributeVertical => arrange::align(
                        &items,
                        match action {
                            AlignLeft => arrange::Align::Left,
                            AlignRight => arrange::Align::Right,
                            AlignTop => arrange::Align::Top,
                            AlignBottom => arrange::Align::Bottom,
                            DistributeHorizontal => arrange::Align::Horizontal,
                            DistributeVertical => arrange::Align::Vertical,
                            _ => arrange::Align::Stack,
                        },
                    ),
                    _ => {
                        if action == ArrangeName {
                            self.tools.order = 1;
                        }
                        if action == ArrangeTime {
                            self.tools.order = 2;
                        }
                        self.tools.arrange(
                            &mut items,
                            match action {
                                Horizontal => arrange::Arrange::Horizontal,
                                Vertical => arrange::Arrange::Vertical,
                                Grid => arrange::Arrange::Grid,
                                _ => arrange::Arrange::Optimal,
                            },
                            &self.settings,
                        )
                    }
                };
                Intent::Commands(commands)
            }
            Lock => {
                self.tools.locked = !self.tools.locked;
                self.workspace.board.locked = self.tools.locked;
                return;
            }
            Grayscale => {
                self.tools.grayscale = !self.tools.grayscale;
                self.workspace.board.grayscale = self.tools.grayscale;
                return;
            }
            MoveDrag | ExtendDrag | ToggleDrag | DuplicateDrag | PanDrag | PanSpaceDrag
            | PanAltDrag => return,
        };
        self.apply(intent, event_loop);
    }
}

impl super::tools::Tools {
    pub fn arrange(
        &mut self,
        items: &mut Vec<&moth_scene::Item>,
        mode: arrange::Arrange,
        settings: &super::settings::Settings,
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
            return arrange::counted(items, mode, self.layout_counts[mode as usize], settings.gap);
        }
        arrange::arrange(
            items,
            mode,
            settings.gap,
            arrange::FreeRectChoice::from_index(settings.arrange_choice),
            arrange::GuillotineSplit::from_index(settings.arrange_split),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::arrange::scale;
    use super::super::{
        settings::Settings,
        tools::{Tools, video_summary},
    };
    use super::*;
    #[test]
    fn saved_zero_gap_restores_gutters_without_spacing_optimal() {
        let mut tools = Tools::default();
        let mut settings = Settings::default();
        settings.load(&[("arrange_gap".into(), "0".into())].into_iter().collect());
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
                .arrange(&mut refs, mode, &settings)
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
        assert_eq!(settings.gap, 24.);
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
            let first = tools.arrange(
                &mut items,
                arrange::Arrange::Horizontal,
                &Settings::default(),
            );
            let reversed = tools.arrange(
                &mut items,
                arrange::Arrange::Horizontal,
                &Settings::default(),
            );
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
        let mut settings = Settings::default();
        for (saved, expected) in [
            ("0", 0),
            ("256", 256),
            ("12288", 12288),
            ("255", 0),
            ("x", 0),
        ] {
            settings.load(
                &[
                    ("vram_limit_mib".into(), saved.into()),
                    ("cache_limit_mib".into(), "512".into()),
                    ("cache_ram_percent".into(), "50".into()),
                ]
                .into_iter()
                .collect(),
            );
            assert_eq!(settings.vram_limit_mib, expected);
            assert!(
                !settings
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
        let source = Settings {
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
            ..Settings::default()
        };
        let map = source
            .preferences()
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        let mut loaded = Settings::default();
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
