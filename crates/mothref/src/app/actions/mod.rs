// SPDX-License-Identifier: GPL-3.0-or-later
//! All registered actions share availability and execution, from keys, buttons
//! or command search.
//!
//! Every action belongs to one [`Area`], whose module answers both "is it
//! available" and "run it", and needs one [`Gate`]: the shared conditions it
//! waits for. Both tables match exhaustively, so a new action does not build
//! until it has an area and a gate.
mod application;
mod drawing;
mod edit;
mod export;
mod group;
mod layout;
mod pointer;
mod prompt;
mod view;
mod window;
use super::*;
use bindings::Action;

/// Where an action's availability and effect live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Area {
    Prompt,
    Application,
    Window,
    View,
    Edit,
    Group,
    Arrange,
    Export,
    Session,
    Pointer,
}
pub(super) fn area(action: Action) -> Area {
    use Action::*;
    match action {
        SaveAndClose | Discard | KeepOpen | CancelDialog | ContinueConversion | SaveConversion
        | SavePath | DismissRecovery | RestoreRecovery | CancelArrange | AddedHorizontal
        | AddedVertical | AddedGrid | AddedOptimal | UndoAdded | DismissAdded => Area::Prompt,
        Palette | Shortcuts | BindingSettings | Settings | New | Open | Images | Save | Library
        | CloseTab | NextTab | CloseWindow | ClearDiskCache | ShellIntegration => Area::Application,
        WindowBar | Overlay | Pin | ClickThrough | OverlaySelection | Bottom | LockWindow
        | ResizeSelection | ResizeAll | OpacityUp | OpacityDown | MoveWindow | Fullscreen
        | Minimize | Maximize => Area::Window,
        Select | Deselect | SelectAll | Fit | Focus | Previous | Next | ZoomIn | ZoomOut
        | ZoomReset | PanLeft | PanRight | PanUp | PanDown | ResetCamera | ToggleView | Lock
        | Grayscale | Layers => Area::View,
        Paste | Undo | Redo | Note | EditNote | Delete | Duplicate | RotateCw | RotateCcw
        | FlipHorizontal | FlipVertical | ResetTransform | ResetCrop | NudgeLeft | NudgeRight
        | NudgeUp | NudgeDown | NudgeLeftFast | NudgeRightFast | NudgeUpFast | NudgeDownFast
        | BringFront | SendBack | BringForward | SendBackward | EditItem | ApplyItem
        | CancelItem | ResetRotation | ResetFlip | ResetScale | NormalizeScale
        | SamplingDefault | SamplingSmooth | SamplingNearest | SamplingAutomatic => Area::Edit,
        Group | Ungroup | EnterGroup | RenameGroup | GroupNeutral | GroupSage | GroupAmber
        | GroupSky | GroupWing | GroupLilac => Area::Group,
        Horizontal | Vertical | Grid | Optimal | ArrangeName | ArrangeTime | PackFit
        | NormalizeWidth | NormalizeHeight | NormalizeSize | NormalizeArea | AlignLeft
        | AlignRight | AlignTop | AlignBottom | Stack | DistributeHorizontal
        | DistributeVertical | ArrangeOptions | ScaleApply => Area::Arrange,
        Export | SampleColor | CoordinatePicker | OpenOriginal | CopyImage | ExportScene
        | ExportSelection | ExportOriginal | ExportOriginals | ExportZip => Area::Export,
        DrawingSession => Area::Session,
        MoveDrag | ExtendDrag | ToggleDrag | DuplicateDrag | PanDrag | PanSpaceDrag
        | PanAltDrag | ZoomDrag => Area::Pointer,
    }
}

/// Shared conditions, each including the ones before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Gate {
    /// Prompts and dialogs answer whatever else is going on.
    None,
    /// Not while a drawing session owns the board.
    NoSession,
    /// Not while a note is being typed.
    NotEditing,
    /// Only on a board, not in the library.
    Board,
    /// Only when the board is idle and not locked.
    Ready,
}
pub(super) fn gate(action: Action) -> Gate {
    use Action::*;
    match action {
        SaveAndClose | Discard | KeepOpen | CancelDialog | ContinueConversion | SaveConversion
        | SavePath | DismissRecovery | RestoreRecovery | CancelArrange | AddedHorizontal
        | AddedVertical | AddedGrid | AddedOptimal | UndoAdded | DismissAdded => Gate::None,
        Layers | DrawingSession | ClickThrough => Gate::NoSession,
        Palette | BindingSettings | Shortcuts | Settings | CloseWindow | New | Open | Images
        | Library | CloseTab | NextTab | Overlay | Minimize | Maximize | Fullscreen | WindowBar
        | ShellIntegration | ClearDiskCache | MoveWindow | LockWindow | OverlaySelection
        | ResizeSelection | ResizeAll | OpacityUp | OpacityDown | Pin | Bottom => Gate::NotEditing,
        PanLeft | PanRight | PanUp | PanDown | Fit | Focus | Previous | Next | ZoomIn | ZoomOut
        | ZoomReset | ResetCamera | Select | ToggleView | Deselect | SelectAll | Lock
        | Grayscale | Save | ExportScene | ExportSelection | ExportOriginal | ExportOriginals
        | ExportZip | CopyImage | OpenOriginal | Export | SampleColor | CoordinatePicker
        | SamplingDefault | SamplingSmooth | SamplingNearest | SamplingAutomatic | EditNote => {
            Gate::Board
        }
        Paste | Undo | Redo | Note | Delete | Duplicate | RotateCw | RotateCcw | FlipHorizontal
        | FlipVertical | ResetTransform | ResetCrop | NudgeLeft | NudgeRight | NudgeUp
        | NudgeDown | NudgeLeftFast | NudgeRightFast | NudgeUpFast | NudgeDownFast | BringFront
        | SendBack | BringForward | SendBackward | EditItem | ApplyItem | CancelItem
        | ResetRotation | ResetFlip | ResetScale | NormalizeScale | Group | Ungroup
        | EnterGroup | RenameGroup | GroupNeutral | GroupSage | GroupAmber | GroupSky
        | GroupWing | GroupLilac | Horizontal | Vertical | Grid | Optimal | ArrangeName
        | ArrangeTime | PackFit | NormalizeWidth | NormalizeHeight | NormalizeSize
        | NormalizeArea | AlignLeft | AlignRight | AlignTop | AlignBottom | Stack
        | DistributeHorizontal | DistributeVertical | ArrangeOptions | ScaleApply | MoveDrag
        | ExtendDrag | ToggleDrag | DuplicateDrag | PanDrag | PanSpaceDrag | PanAltDrag
        | ZoomDrag => Gate::Ready,
    }
}
impl App {
    /// Whether `action` can run now: its [`Gate`] first, then its area's own rule.
    pub(super) fn action_enabled(&self, action: Action) -> bool {
        let board = &self.workspace.board;
        let gate = gate(action);
        let blocked = (gate >= Gate::NoSession && board.session.is_some())
            || (gate >= Gate::NotEditing && self.editing.is_some())
            || (gate >= Gate::Board && self.workspace.library)
            || (gate >= Gate::Ready && (!board.idle() || self.workspace.board.locked));
        !blocked
            && match area(action) {
                Area::Prompt => self.prompt_enabled(action),
                Area::Application => self.application_enabled(action),
                Area::Window => self.window_enabled(action),
                Area::View => self.view_enabled(action),
                Area::Edit => self.edit_enabled(action),
                Area::Group => self.group_enabled(action),
                Area::Arrange => self.arrange_enabled(action),
                Area::Export => self.export_enabled(action),
                Area::Session => self.session_enabled(action),
                Area::Pointer => self.pointer_enabled(action),
            }
    }
    pub(super) fn run_action(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        if !self.action_enabled(action) {
            return;
        }
        let intent = match area(action) {
            Area::Prompt => self.run_prompt(action, event_loop),
            Area::Application => self.run_application(action, event_loop),
            Area::Window => self.run_window(action, event_loop),
            Area::View => self.run_view(action, event_loop),
            Area::Edit => self.run_edit(action, event_loop),
            Area::Group => self.run_group(action, event_loop),
            Area::Arrange => self.run_arrange(action, event_loop),
            Area::Export => self.run_export(action, event_loop),
            Area::Session => self.run_session(action, event_loop),
            Area::Pointer => self.run_pointer(action, event_loop),
        };
        if let Some(intent) = intent {
            self.apply(intent, event_loop);
        }
    }
    /// The selected items in stacking order. A group's frame follows its members,
    /// so only actions that act on whole groups keep them.
    fn selected_items(board: &Document, action: Action) -> Vec<&moth_scene::Item> {
        use Action::*;
        let mut items: Vec<_> = board
            .selection
            .iter()
            .filter_map(|id| board.scene.get(*id))
            .collect();
        items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
        if !matches!(
            action,
            Delete
                | Duplicate
                | ExportSelection
                | NudgeLeft
                | NudgeRight
                | NudgeUp
                | NudgeDown
                | NudgeLeftFast
                | NudgeRightFast
                | NudgeUpFast
                | NudgeDownFast
        ) {
            items.retain(|item| !item.is_group());
        }
        items
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
        self.arrange.layout_counts.hash(&mut hash);
        let key = (self.arrange.order, mode as u8, hash.finish());
        let repeat = self.arrange.last_arrange == Some(key);
        self.arrange.reversed = repeat && !self.arrange.reversed;
        self.arrange.last_arrange = Some(key);
        if !repeat {
            self.arrange.random_salt = Uuid::now_v7().as_u128();
        }
        match self.arrange.order {
            1 => items.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id))),
            2 => items.sort_by_key(|i| i.id),
            3 => items.sort_by_key(|i| {
                i.id.as_u128()
                    .wrapping_mul(self.arrange.random_salt | 1)
                    .rotate_left(43)
            }),
            _ => items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id))),
        }
        if self.arrange.reversed {
            items.reverse();
        }
        if matches!(
            mode,
            arrange::Arrange::Horizontal | arrange::Arrange::Vertical
        ) {
            return arrange::counted(
                items,
                mode,
                self.arrange.layout_counts[mode as usize],
                settings.gap,
            );
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
            let mut tools = Tools::default();
            tools.arrange.order = order;
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
