// SPDX-License-Identifier: GPL-3.0-or-later
//! Owns library widgets and their bounded preview texture handles.
pub(super) struct LibraryScreen {
    pub worker: moth_platform::library::Worker,
    pub cards: Vec<moth_ui::library::Card>,
    pub catalog: moth_ui::library::Library,
    /// The app folders, as the library worker last reported them.
    pub folders: Vec<moth_ui::library::Folder>,
    /// "Find…" for a missing board: its old path and the system dialog's answer.
    pub relink: Option<(
        std::path::PathBuf,
        crossbeam_channel::Receiver<Option<std::path::PathBuf>>,
    )>,
    /// A relink sent to the worker: its answer moves the board in the recent list too.
    pub relinking: Option<(std::path::PathBuf, std::path::PathBuf)>,
}
impl LibraryScreen {
    pub(super) fn new(worker: moth_platform::library::Worker) -> Self {
        Self {
            worker,
            cards: Vec::new(),
            catalog: Default::default(),
            folders: Vec::new(),
            relink: None,
            relinking: None,
        }
    }
}
