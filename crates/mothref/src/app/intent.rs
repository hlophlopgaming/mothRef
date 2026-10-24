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
    /// Toggle the board tools window.
    ToggleTools,
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
    /// Defer a fit until the canvas lays out next frame.
    NeedsFit,
    /// Relative zoom from buttons and shortcuts.
    Zoom(f32),
    /// Exact scale replaces any pending animated zoom target.
    ZoomTo(f32),
    /// Return to the catalog without closing the board.
    Home,
    /// Create a bounded new board.
    New,
    /// Open the platform file dialog.
    Open,
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
    /// Persist changed preferences and re-apply the theme.
    PreferencesDirty,
    /// Dismiss the pending arrangement prompt without arranging.
    DismissArrangePrompt,
    /// Toggle the focus-return camera for an item, fitting to it on first focus.
    FocusItem(Uuid),
    /// Sync the per-tab lock and grayscale view flags from the tools panel.
    SetViewFlags { locked: bool, grayscale: bool },
    /// Dismiss the recovery browser without opening a board.
    DismissRecovery,
}

/// Converts the shared chrome action set into intents in application order.
pub(super) fn intents_from_actions(actions: Actions) -> Vec<Intent> {
    let mut intents = Vec::new();
    if actions.paste {
        intents.push(Intent::Paste);
    }
    if actions.fit {
        intents.push(Intent::Fit);
    }
    if actions.zoom > 0. {
        intents.push(Intent::Zoom(actions.zoom));
    }
    if let Some(zoom) = actions.zoom_to {
        intents.push(Intent::ZoomTo(zoom));
    }
    if actions.home {
        intents.push(Intent::Home);
    }
    if actions.new {
        intents.push(Intent::New);
    }
    if actions.open {
        intents.push(Intent::Open);
    }
    if actions.save {
        intents.push(Intent::Save);
    }
    if actions.undo {
        intents.push(Intent::Undo);
    }
    if actions.redo {
        intents.push(Intent::Redo);
    }
    if actions.overlay {
        intents.push(Intent::Overlay);
    }
    if let Some(target) = actions.pin_choice {
        intents.push(Intent::PinChoice(target));
    }
    if actions.pin {
        intents.push(Intent::Pin);
    }
    if let Some(index) = actions.tab {
        intents.push(Intent::Tab(index));
    }
    if let Some(index) = actions.close_tab {
        intents.push(Intent::CloseTab(index));
    }
    if actions.close {
        intents.push(Intent::CloseWindow);
    }
    if actions.drag {
        intents.push(Intent::Drag);
    }
    if actions.minimize {
        intents.push(Intent::Minimize);
    }
    if actions.maximize {
        intents.push(Intent::Maximize);
    }
    if let Some(language) = actions.language {
        intents.push(Intent::Language(language));
    }
    intents
}
