// SPDX-License-Identifier: GPL-3.0-or-later
//! User intent collected from the UI pass, applied in one place after it.
//!
//! UI sections never mutate board state directly; they emit intents that `App`
//! applies in order. A single `Intent` maps to one side effect, so close-with-save
//! ordering is expressed by the variant rather than by vector order.
use std::path::PathBuf;

use moth_platform::export::{Geometry, Target};
use moth_scene::{Command, Uuid};
use moth_ui::{Actions, Locale};

use super::arrange::Arrange;

/// One user action from chrome, canvas, tools or a modal.
pub(super) enum Intent {
    /// Execute the same registered action used by keyboard dispatch and command search.
    Action(super::bindings::Action),
    /// Commit the locally laid-out canvas before applying its user actions.
    View(super::document::View),
    Appearance(super::window_mode::Appearance),
    /// Acknowledge the inline editor focus request.
    NoteFocused,
    /// Acknowledge the save-path widget focus request.
    DialogFocused,
    /// Send an explicit catalog action to the library worker.
    Library(moth_platform::library::Request),

    /// Clipboard import.
    Paste,
    /// Fit the camera to the whole board immediately.
    Fit,
    /// Relative zoom from buttons and shortcuts.
    Zoom(f32),
    /// Exact scale replaces any pending animated zoom target.
    ZoomTo(f32),
    /// Move the viewport in screen points, independent of zoom.
    Pan([f32; 2]),
    /// Return to the catalog without closing the board.
    Home,
    /// Create a bounded new board.
    New,
    /// Open the platform file dialog.
    Open,
    /// Browse for images to insert into the current board.
    Images,
    /// Save the active board, prompting for a path first if needed.
    Save,
    /// Journal undo.
    Undo,
    /// Journal redo.
    Redo,
    /// Toggle transparent overlay viewing.
    Overlay,
    /// Toggle native topmost.
    Pin,
    /// Choose a pin mode.
    PinChoice(bool),
    /// Cancel target selection or an armed window drag.
    CancelWindowMode,
    /// Toggle borderless fullscreen.
    Fullscreen,
    /// Activate an inactive tab.
    Tab(usize),
    /// Request safe closure of a tab (zero is the active board).
    CloseTab(usize),
    /// Close the window after all tabs are handled.
    CloseWindow,
    /// Native titlebar drag.
    Drag,
    /// Show or hide the View window bar explicitly (touch, context menu).
    WindowBar(bool),
    /// Move a tab into a new window (or onto another window's tab strip when a
    /// drag ended at that position).
    DetachTab(usize, Option<[f32; 2]>),
    /// Move a tab into another existing window.
    MoveTab(usize, usize),
    /// Bring every other window's tabs into this one.
    GatherTabs,
    /// Native minimize.
    Minimize,
    /// Native maximize toggle.
    Maximize,
    /// Switch interface language.
    Language(Locale),
    /// Open a specific path (catalog card, recovery entry, dropped file).
    OpenPath(PathBuf),
    /// The save-as dialog committed a path.
    SaveAs(PathBuf),
    /// Dismiss the save-as dialog without saving.
    CancelDialog,
    /// Save the active board, then discard once the save completes.
    SaveAndClose,
    /// Discard the active board's staged originals and journal.
    Discard,
    /// Leave the close confirmation open, abandoning the pending close.
    KeepOpen,
    /// Create a note at a world position and open its editor.
    AddNote([f32; 2]),
    /// Open the inline editor over an existing note.
    EditNote(Uuid),
    /// Commit or discard the inline editor's draft.
    CommitNote,
    /// Journal a whole command batch as one undo step.
    Commands(Vec<Command>),
    /// Lay a finished import burst out with a chosen arrangement.
    Arrange(Arrange, Vec<Uuid>),
    /// Fit the camera to the current selection.
    FitSelection,
    /// Export through the storage worker.
    Export(Target),
    /// Reset content geometry (crop/scale) on a set of items.
    OriginalGeometry(Vec<Uuid>, Geometry),
    /// Reset the camera to its origin.
    ResetCamera,
    /// Step selection focus to the previous/next image.
    Focus(i32),
    /// Open a schema row found through command search.
    SettingsRow(usize),
    /// Persist changed preferences and re-apply the theme.
    PreferencesDirty,
    /// Dismiss the pending arrangement prompt without arranging.
    DismissArrangePrompt,
    /// Toggle the focus-return camera for an item, fitting to it on first focus.
    FocusItem(Uuid),
    /// Dismiss the recovery browser without opening a board.
    DismissRecovery,
    /// Restore the selected recovery entry through its storage worker.
    RestoreRecovery,
}

/// Converts the shared chrome action set into intents in application order.
pub(super) fn intents_from_actions(actions: Actions) -> Vec<Intent> {
    let mut intents = Vec::new();
    if actions.settings {
        intents.push(Intent::Action(super::bindings::Action::Settings));
    }
    if let Some((slot, at)) = actions.detach_tab {
        intents.push(Intent::DetachTab(slot, at));
    }
    if let Some((slot, window)) = actions.move_tab {
        intents.push(Intent::MoveTab(slot, window));
    }
    if actions.gather {
        intents.push(Intent::GatherTabs);
    }
    if actions.paste {
        intents.push(Intent::Action(super::bindings::Action::Paste));
    }
    if actions.fit {
        intents.push(Intent::Action(super::bindings::Action::Fit));
    }
    if actions.zoom > 0. {
        intents.push(Intent::Zoom(actions.zoom));
    }
    if let Some(zoom) = actions.zoom_to {
        intents.push(Intent::ZoomTo(zoom));
    }
    if actions.home {
        intents.push(Intent::Action(super::bindings::Action::Library));
    }
    if actions.new {
        intents.push(Intent::Action(super::bindings::Action::New));
    }
    if actions.open {
        intents.push(Intent::Action(super::bindings::Action::Open));
    }
    if actions.save {
        intents.push(Intent::Action(super::bindings::Action::Save));
    }
    if actions.undo {
        intents.push(Intent::Action(super::bindings::Action::Undo));
    }
    if actions.redo {
        intents.push(Intent::Action(super::bindings::Action::Redo));
    }
    if actions.overlay {
        intents.push(Intent::Action(super::bindings::Action::Overlay));
    }
    if let Some(target) = actions.pin_choice {
        intents.push(Intent::PinChoice(target));
    }
    if actions.pin {
        intents.push(Intent::Action(super::bindings::Action::Pin));
    }
    if let Some(index) = actions.tab {
        intents.push(Intent::Tab(index));
    }
    if let Some(index) = actions.close_tab {
        intents.push(Intent::CloseTab(index));
    }
    if actions.close {
        intents.push(Intent::Action(super::bindings::Action::CloseWindow));
    }
    if actions.drag {
        intents.push(Intent::Drag);
    }
    if actions.minimize {
        intents.push(Intent::Action(super::bindings::Action::Minimize));
    }
    if actions.maximize {
        intents.push(Intent::Action(super::bindings::Action::Maximize));
    }
    if let Some(language) = actions.language {
        intents.push(Intent::Language(language));
    }
    intents
}
