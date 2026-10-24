// SPDX-License-Identifier: GPL-3.0-or-later
//! One bounded action inventory for dispatch, persisted chords and every shortcut label.
use egui::Key;
use moth_ui::Text;
use std::collections::{BTreeMap, HashSet};

macro_rules! actions {
    ($($id:ident, $label:ident, $default:literal, $icon:expr;)+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(super) enum Action { $($id,)+ }
        pub(super) const ACTIONS: &[Action] = &[$(Action::$id,)+];
        impl Action {
            pub fn id(self) -> &'static str { match self { $(Self::$id => stringify!($id),)+ } }
            pub fn label(self) -> Text { match self { $(Self::$id => Text::$label,)+ } }
            pub fn icon(self) -> Option<moth_ui::icons::Icon> { match self { $(Self::$id => $icon,)+ } }
            fn default(self) -> &'static str { match self { $(Self::$id => $default,)+ } }
        }
    }
}
actions! {
    Palette, CommandPalette, "Cmd+K", None;
    Shortcuts, CheatSheet, "Slash", None;
    BindingSettings, Shortcuts, "", None;
    New, NewBoard, "Cmd+N", Some(moth_ui::icons::Icon::Plus);
    Open, Open, "Cmd+O", Some(moth_ui::icons::Icon::Folder);
    Save, Save, "Cmd+S", Some(moth_ui::icons::Icon::Save);
    Library, Library, "Cmd+L", Some(moth_ui::icons::Icon::Home);
    CloseTab, CloseTab, "Cmd+W", Some(moth_ui::icons::Icon::Close);
    NextTab, NextTab, "Cmd+Tab", None;
    Paste, Paste, "Cmd+V", Some(moth_ui::icons::Icon::Paste);
    Undo, Undo, "Cmd+Z", Some(moth_ui::icons::Icon::Undo);
    Redo, Redo, "Cmd+Y", Some(moth_ui::icons::Icon::Redo);
    Overlay, Overlay, "Cmd+T", Some(moth_ui::icons::Icon::Overlay);
    Pin, OnTop, "Cmd+Shift+T", Some(moth_ui::icons::Icon::Pin);
    MoveWindow, MoveWindow, "Cmd+Shift+M", None;
    Fullscreen, Fullscreen, "F11", None;
    Minimize, Minimize, "", Some(moth_ui::icons::Icon::Minus);
    Maximize, Maximize, "", Some(moth_ui::icons::Icon::Maximize);
    Settings, Settings, "Cmd+Comma", Some(moth_ui::icons::Icon::List);
    Select, Select, "V", Some(moth_ui::icons::Icon::Select);
    Hand, Pan, "H", Some(moth_ui::icons::Icon::Pan);
    Fit, Fit, "Home", Some(moth_ui::icons::Icon::Fit);
    Focus, FitSelection, "Space", Some(moth_ui::icons::Icon::Fit);
    Previous, PreviousImage, "PageUp", None;
    Next, NextImage, "PageDown", None;
    ZoomIn, ZoomIn, "Equal", Some(moth_ui::icons::Icon::Plus);
    ZoomOut, ZoomOut, "Minus", Some(moth_ui::icons::Icon::Minus);
    ZoomReset, ZoomReset, "", None;
    ResetCamera, ResetCamera, "", None;
    Note, AddNote, "", Some(moth_ui::icons::Icon::Plus);
    Delete, Delete, "Delete", None;
    Duplicate, Duplicate, "Cmd+D", None;
    RotateCw, RotateCw, "R", None;
    RotateCcw, RotateCcw, "Shift+R", None;
    FlipHorizontal, FlipHorizontal, "Shift+H", None;
    FlipVertical, FlipVertical, "Shift+V", None;
    ResetTransform, ResetTransform, "Shift+0", None;
    ResetCrop, ResetCrop, "", None;
    NudgeLeft, NudgeLeft, "ArrowLeft", None;
    NudgeRight, NudgeRight, "ArrowRight", None;
    NudgeUp, NudgeUp, "ArrowUp", None;
    NudgeDown, NudgeDown, "ArrowDown", None;
    NudgeLeftFast, NudgeLeftFast, "Shift+ArrowLeft", None;
    NudgeRightFast, NudgeRightFast, "Shift+ArrowRight", None;
    NudgeUpFast, NudgeUpFast, "Shift+ArrowUp", None;
    NudgeDownFast, NudgeDownFast, "Shift+ArrowDown", None;
    BringFront, BringFront, "", None;
    SendBack, SendBack, "", None;
    BringForward, BringForward, "", None;
    SendBackward, SendBackward, "", None;
    Horizontal, Horizontal, "", Some(moth_ui::icons::Icon::ArrangeHorizontal);
    Vertical, Vertical, "", Some(moth_ui::icons::Icon::ArrangeVertical);
    Grid, Grid, "", Some(moth_ui::icons::Icon::ArrangeGrid);
    Optimal, Optimal, "", Some(moth_ui::icons::Icon::ArrangeOptimal);
    ArrangeName, ArrangeName, "", None;
    ArrangeTime, ArrangeTime, "", None;
    PackFit, PackFit, "", Some(moth_ui::icons::Icon::ArrangeOptimal);
    NormalizeWidth, NormalizeWidth, "", None;
    NormalizeHeight, NormalizeHeight, "", None;
    NormalizeSize, NormalizeSize, "", None;
    NormalizeArea, NormalizeArea, "", None;
    AlignLeft, AlignLeft, "", None;
    AlignRight, AlignRight, "", None;
    AlignTop, AlignTop, "", None;
    AlignBottom, AlignBottom, "", None;
    Stack, Stack, "", None;
    Lock, LockCanvas, "", None;
    Grayscale, CanvasGrayscale, "", None;
    MoveDrag, MoveDrag, "Mouse1", None;
    ExtendDrag, ExtendDrag, "Shift+Mouse1", None;
    DuplicateDrag, DuplicateDrag, "Alt+Mouse1", None;
    PanDrag, PanDrag, "Mouse3", None;
    PanSpaceDrag, PanSpaceDrag, "Space+Mouse1", None;
    PanAltDrag, PanAltDrag, "", None;
    SaveAndClose, SaveClose, "", Some(moth_ui::icons::Icon::Save);
    Discard, Close, "", Some(moth_ui::icons::Icon::Close);
    KeepOpen, Keep, "", None;
    CancelDialog, Cancel, "", None;
    SavePath, Save, "", Some(moth_ui::icons::Icon::Save);
    DismissRecovery, Keep, "", None;
    CancelArrange, Cancel, "", None;
    CloseWindow, CloseWindow, "", Some(moth_ui::icons::Icon::Close);
    EditNote, EditNote, "", Some(moth_ui::icons::Icon::List);
    EditItem, EditItem, "", Some(moth_ui::icons::Icon::List);
    ApplyItem, ApplyItem, "", None;
    CancelItem, CancelItem, "", None;
    ArrangeOptions, ArrangeOptions, "", Some(moth_ui::icons::Icon::ArrangeOptimal);
    ScaleApply, ScaleApply, "", None;
    DistributeHorizontal, DistributeHorizontal, "", None;
    DistributeVertical, DistributeVertical, "", None;
    ResetRotation, ResetRotation, "", None;
    ResetFlip, ResetFlip, "", None;
    ResetScale, ResetScale, "", None;
    NormalizeScale, NormalizeScale, "", None;
    SampleColor, SampleColor, "", Some(moth_ui::icons::Icon::Pointer);
    CoordinatePicker, CoordinatePicker, "", Some(moth_ui::icons::Icon::Pointer);
    Export, Export, "", Some(moth_ui::icons::Icon::Folder);
    ExportScene, ExportScene, "", None;
    ExportSelection, ExportSelection, "", None;
    ExportOriginal, ExportOriginal, "", None;
    ExportOriginals, ExportOriginals, "", None;
    ExportZip, ExportZip, "", None;
    CopyImage, CopyImage, "", None;
    ClearDiskCache, ClearDiskCache, "", Some(moth_ui::icons::Icon::Close);

}
impl Action {
    pub fn category(self) -> Text {
        use Action::*;
        match self {
            MoveDrag | ExtendDrag | DuplicateDrag | PanDrag | PanSpaceDrag | PanAltDrag => {
                Text::CategoryPointer
            }
            Select | Hand | Fit | Focus | Previous | Next | ZoomIn | ZoomOut | ZoomReset
            | ResetCamera | Lock | Grayscale => Text::CategoryView,
            Palette | BindingSettings | Shortcuts | New | Open | Save | Library | CloseTab
            | NextTab | Overlay | Pin | Minimize | Maximize | MoveWindow | Fullscreen
            | Settings | CloseWindow => Text::CategoryBoard,
            _ => Text::CategoryEdit,
        }
    }
    pub fn pointer(self) -> bool {
        matches!(
            self,
            Self::MoveDrag
                | Self::ExtendDrag
                | Self::DuplicateDrag
                | Self::PanDrag
                | Self::PanSpaceDrag
                | Self::PanAltDrag
        )
    }
    pub fn repeat(self) -> bool {
        matches!(
            self,
            Self::NudgeLeft
                | Self::NudgeRight
                | Self::NudgeUp
                | Self::NudgeDown
                | Self::NudgeLeftFast
                | Self::NudgeRightFast
                | Self::NudgeUpFast
                | Self::NudgeDownFast
                | Self::ZoomIn
                | Self::ZoomOut
        )
    }
}

/// Portable primary command modifier, plus independently matched Shift and Alt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Mods {
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
    pub control: bool,
    pub super_key: bool,
}
impl Mods {
    pub fn from_egui(m: egui::Modifiers) -> Self {
        Self {
            command: m.command,
            shift: m.shift,
            alt: m.alt,
            control: m.ctrl && (!m.command || m.mac_cmd),
            super_key: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Trigger {
    Key(Key),
    Drag(u8, Option<Key>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Chord {
    pub mods: Mods,
    pub trigger: Trigger,
}
impl Chord {
    pub fn parse(value: &str) -> Option<Self> {
        let mut mods = Mods::default();
        let mut key = None;
        let mut button = None;
        for part in value.split('+') {
            match part {
                "Cmd" if !mods.command => mods.command = true,
                "Shift" if !mods.shift => mods.shift = true,
                "Alt" if !mods.alt => mods.alt = true,
                "Super" if !mods.super_key => {
                    if egui::os::OperatingSystem::default() == egui::os::OperatingSystem::Mac {
                        if mods.command {
                            return None;
                        }
                        mods.command = true;
                    } else {
                        mods.super_key = true;
                    }
                }
                "Ctrl" if !mods.control => {
                    if egui::os::OperatingSystem::default() == egui::os::OperatingSystem::Mac {
                        mods.control = true;
                    } else if !mods.command {
                        mods.command = true;
                    } else {
                        return None;
                    }
                }
                "Mouse1" | "Mouse2" | "Mouse3" if button.is_none() => {
                    button = part[5..].parse().ok()
                }
                _ if key.is_none() => key = Some(Key::from_name(part)?),
                _ => return None,
            }
        }
        let trigger = match (key, button) {
            (key, Some(button)) if key != Some(Key::Escape) => Trigger::Drag(button, key),
            (Some(key), None) if key != Key::Escape => Trigger::Key(key),
            _ => return None,
        };
        Some(Self { mods, trigger })
    }
    pub fn encode(self) -> String {
        let mut parts = Vec::new();
        if self.mods.command {
            parts.push("Cmd".to_owned());
        }
        if self.mods.control {
            parts.push("Ctrl".to_owned());
        }
        if self.mods.super_key {
            parts.push("Super".to_owned());
        }
        if self.mods.shift {
            parts.push("Shift".to_owned());
        }
        if self.mods.alt {
            parts.push("Alt".to_owned());
        }
        match self.trigger {
            Trigger::Key(key) => parts.push(key.name().to_owned()),
            Trigger::Drag(button, key) => {
                if let Some(key) = key {
                    parts.push(key.name().to_owned());
                }
                parts.push(format!("Mouse{button}"));
            }
        }
        parts.join("+")
    }
    pub fn display(self) -> String {
        self.encode().replace(
            "Cmd",
            if egui::os::OperatingSystem::default() == egui::os::OperatingSystem::Mac {
                "⌘"
            } else {
                "Ctrl"
            },
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Registry {
    pub entries: Vec<Option<Chord>>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            entries: ACTIONS.iter().map(|a| Chord::parse(a.default())).collect(),
        }
    }
}
impl Registry {
    pub fn get(&self, action: Action) -> Option<Chord> {
        self.entries[action as usize]
    }
    pub fn set(&mut self, action: Action, value: &str) {
        self.entries[action as usize] = Chord::parse(value);
    }
    pub fn label(&self, action: Action) -> String {
        self.get(action)
            .map_or_else(String::new, |chord| chord.display().replace("Slash", "/"))
    }
    pub fn conflicts(&self) -> Vec<(Action, Action)> {
        let mut conflicts = Vec::new();
        for (index, a) in ACTIONS.iter().enumerate() {
            if let Some(chord) = self.get(*a) {
                for b in &ACTIONS[index + 1..] {
                    if let Some(other) = self.get(*b) {
                        let overlap = chord.mods == other.mods
                            && match (chord.trigger, other.trigger) {
                                (Trigger::Key(key), Trigger::Drag(_, Some(held))) => {
                                    key == held && *a != Action::Focus
                                }
                                (Trigger::Drag(_, Some(held)), Trigger::Key(key)) => {
                                    key == held && *b != Action::Focus
                                }
                                _ => false,
                            };
                        if chord == other || overlap {
                            conflicts.push((*a, *b));
                        }
                    }
                }
            }
        }
        conflicts
    }
    pub fn key(&self, key: Key, mods: Mods) -> Option<Action> {
        ACTIONS.iter().copied().find(|a| {
            self.get(*a)
                == Some(Chord {
                    mods,
                    trigger: Trigger::Key(key),
                })
        })
    }
    pub fn pointer(&self, button: u8, mods: Mods, held: &HashSet<Key>) -> Option<Action> {
        ACTIONS.iter().copied().find(|a| {
            self.get(*a).is_some_and(|chord| {
                chord.mods == mods
                    && matches!(chord.trigger, Trigger::Drag(b, key) if b == button && match key {
                        Some(key) => held.len() == 1 && held.contains(&key),
                        None => held.is_empty(),
                    })
            })
        })
    }
    pub fn preferences(&self) -> BTreeMap<String, String> {
        ACTIONS
            .iter()
            .map(|a| {
                (
                    format!("binding_{}", a.id()),
                    self.get(*a).map_or_else(String::new, Chord::encode),
                )
            })
            .collect()
    }
    pub fn load(&mut self, values: &BTreeMap<String, String>) -> bool {
        let mut candidate = Self::default();
        for action in ACTIONS {
            if let Some(value) = values.get(&format!("binding_{}", action.id())) {
                let chord = if value.is_empty() {
                    None
                } else {
                    let Some(chord) = Chord::parse(value) else {
                        return false;
                    };
                    if action.pointer() != matches!(chord.trigger, Trigger::Drag(..)) {
                        return false;
                    }
                    Some(chord)
                };
                candidate.entries[*action as usize] = chord;
            }
        }
        if !candidate.conflicts().is_empty() {
            return false;
        }
        *self = candidate;
        true
    }
    /// Navigation-only compatibility presets never claim missing screenshot actions.
    pub fn preset(&self, pure: bool) -> Self {
        let mut next = self.clone();
        for (a, chord) in [
            (Action::PanSpaceDrag, "Space+Mouse1"),
            (Action::Focus, "Space"),
            (Action::PanDrag, "Mouse3"),
        ] {
            next.set(a, chord);
        }
        if pure {
            for (a, chord) in [
                (Action::PanAltDrag, "Alt+Mouse1"),
                (Action::DuplicateDrag, "Cmd+Alt+Mouse1"),
                (Action::Previous, "ArrowLeft"),
                (Action::Next, "ArrowRight"),
                (Action::BringFront, "ArrowUp"),
                (Action::SendBack, "ArrowDown"),
                (Action::NudgeLeft, "Alt+ArrowLeft"),
                (Action::NudgeRight, "Alt+ArrowRight"),
                (Action::NudgeUp, "Alt+ArrowUp"),
                (Action::NudgeDown, "Alt+ArrowDown"),
            ] {
                next.set(a, chord);
            }
        } else {
            for a in [
                Action::PanAltDrag,
                Action::DuplicateDrag,
                Action::NudgeLeft,
                Action::NudgeRight,
                Action::NudgeUp,
                Action::NudgeDown,
                Action::Previous,
                Action::Next,
                Action::BringFront,
                Action::SendBack,
            ] {
                next.set(a, a.default());
            }
        }
        next
    }
    pub fn publish(&self, ctx: &egui::Context) {
        moth_ui::set_shortcuts(
            ctx,
            ACTIONS
                .iter()
                .map(|a| (a.label(), self.label(*a)))
                .collect(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_actions_rebind_and_survive_settings_roundtrip() {
        let mut registry = Registry::default();
        for (action, chord) in [
            (Action::Pin, "Alt+T"),
            (Action::MoveWindow, "Alt+M"),
            (Action::Fullscreen, "Alt+F11"),
        ] {
            registry.set(action, chord);
        }
        let mut restored = Registry::default();
        assert!(restored.load(&registry.preferences()));
        assert_eq!(restored, registry);
        let alt = Mods {
            alt: true,
            ..Mods::default()
        };
        assert_eq!(restored.key(Key::T, alt), Some(Action::Pin));
        assert_eq!(restored.key(Key::M, alt), Some(Action::MoveWindow));
        assert_eq!(restored.key(Key::F11, alt), Some(Action::Fullscreen));
    }
    #[test]
    fn exact_modifiers_do_not_turn_screenshot_chords_into_other_commands() {
        let registry = Registry::default();
        let cmd = Mods {
            command: true,
            ..Mods::default()
        };
        assert_eq!(registry.key(Key::N, cmd), Some(Action::New));
        assert_eq!(registry.key(Key::N, Mods { alt: true, ..cmd }), None);
        assert_eq!(registry.key(Key::N, Mods { shift: true, ..cmd }), None);
        assert_eq!(registry.key(Key::K, cmd), Some(Action::Palette));
        assert_eq!(
            registry.key(Key::ArrowUp, Mods::default()),
            Some(Action::NudgeUp)
        );
        assert_eq!(registry.key(Key::ArrowUp, cmd), None);
    }
    #[test]
    fn presets_resolve_navigation_ambiguities_and_preview_remaining_conflicts() {
        let mut registry = Registry::default();
        assert!(registry.conflicts().is_empty());
        for action in ACTIONS {
            assert!(
                action.default().is_empty() || registry.get(*action).is_some(),
                "invalid default: {}",
                action.id()
            );
        }
        let pure = registry.preset(true);
        assert!(pure.conflicts().is_empty());
        assert_eq!(
            pure.key(Key::ArrowUp, Mods::default()),
            Some(Action::BringFront)
        );
        assert_eq!(
            pure.pointer(
                1,
                Mods {
                    alt: true,
                    ..Mods::default()
                },
                &HashSet::new()
            ),
            Some(Action::PanAltDrag)
        );
        assert_eq!(pure.key(Key::Space, Mods::default()), Some(Action::Focus));
        assert_eq!(
            pure.pointer(1, Mods::default(), &HashSet::from([Key::Space])),
            Some(Action::PanSpaceDrag)
        );
        let mut overlap = registry.clone();
        overlap.set(Action::PanSpaceDrag, "R+Mouse1");
        assert!(
            overlap
                .conflicts()
                .contains(&(Action::RotateCw, Action::PanSpaceDrag))
        );
        registry.set(Action::Save, "Alt+Mouse1");
        assert!(!registry.preset(true).conflicts().is_empty());
    }
    #[test]
    fn settings_roundtrip_and_reject_corruption_without_partial_changes() {
        let mut registry = Registry::default().preset(true);
        let mut loaded = Registry::default();
        assert!(loaded.load(&registry.preferences()));
        assert_eq!(loaded, registry);
        let original = registry.clone();
        let mut values = registry.preferences();
        values.insert("binding_Save".into(), "Cmd+N".into());
        assert!(!registry.load(&values));
        assert_eq!(registry, original);
        values.insert("binding_Save".into(), "bogus".into());
        assert!(!registry.load(&values));
        for action in ACTIONS {
            if let Some(chord) = original.get(*action) {
                assert_eq!(Chord::parse(&chord.encode()), Some(chord));
            }
        }
    }
}
