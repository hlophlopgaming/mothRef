// SPDX-License-Identifier: GPL-3.0-or-later
//! Owns the active board, its parked tabs and the recovery/recent/startup paths.
use super::*;

/// Owns the tab set and the close flow so switching tabs never exchanges
/// persistence identities (see `Session`).
pub(super) struct Workspace {
    pub(super) board: Session,
    pub(super) tabs: Vec<Session>,
    pub(super) screen: Screen,
    pub(super) closing_window: bool,
    pub(super) recovery_paths: Vec<std::path::PathBuf>,
    pub(super) recent_paths: Vec<std::path::PathBuf>,
    pub(super) startup: Vec<std::path::PathBuf>,
}
