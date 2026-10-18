// SPDX-License-Identifier: GPL-3.0-or-later
//! Owns library widgets and their bounded preview texture handles.
pub(super) struct LibraryScreen {
    pub worker: moth_platform::library::Worker,
    pub cards: Vec<moth_ui::library::Card>,
    pub catalog: moth_ui::library::Library,
}
impl LibraryScreen {
    pub(super) fn new(worker: moth_platform::library::Worker) -> Self {
        Self {
            worker,
            cards: Vec::new(),
            catalog: Default::default(),
        }
    }
}
