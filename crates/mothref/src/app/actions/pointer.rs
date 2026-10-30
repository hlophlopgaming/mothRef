// SPDX-License-Identifier: GPL-3.0-or-later
//! Pointer bindings: they configure gestures and do nothing when run.
use super::*;
impl App {
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn pointer_enabled(&self, _action: Action) -> bool {
        !self.workspace.board.selection.is_empty()
    }
    /// Runs an action of this area: never anything, gestures read the binding.
    pub(super) fn run_pointer(
        &mut self,
        _action: Action,
        _event_loop: &ActiveEventLoop,
    ) -> Option<Intent> {
        None
    }
}
