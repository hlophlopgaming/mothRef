// SPDX-License-Identifier: GPL-3.0-or-later
//! Starting a timed drawing session.
use super::*;
impl App {
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn session_enabled(&self, _action: Action) -> bool {
        !self.workspace.library
            && self.workspace.board.idle()
            && self.workspace.board.image_count > 0
    }
    /// Opens the session setup over the board, preset to the selected group, the
    /// selected images or the whole board.
    pub(super) fn run_session(
        &mut self,
        _action: Action,
        _event_loop: &ActiveEventLoop,
    ) -> Option<Intent> {
        let board = &mut self.workspace.board;
        let mut setup = self
            .window_preferences
            .get("session_setup")
            .map(|value| session::Setup::decode(value))
            .unwrap_or_default();
        let has_images = board
            .selection
            .iter()
            .any(|id| board.scene.get(*id).is_some_and(|i| i.blob().is_some()));
        setup.source =
            match selection::single_group(&board.scene, &board.selection).or(board.entered) {
                Some(group) => session::Source::Group(group),
                None if has_images => session::Source::Selection,
                None => session::Source::Board,
            };
        board.session = Some(session_ui::State::new(setup));
        self.cancel_drag();
        None
    }
}
