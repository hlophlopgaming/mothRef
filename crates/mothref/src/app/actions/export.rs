// SPDX-License-Identifier: GPL-3.0-or-later
//! Export, the image pickers and opening originals elsewhere.
use super::*;
impl App {
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn export_enabled(&self, action: Action) -> bool {
        use Action::*;
        // The destination is chosen in the system dialog once the action runs, so only
        // an export already waiting for one, or an invalid crop, holds the actions back.
        let export_ready = self.window.export_picker.is_none()
            && (!self.tools.export.crop || self.tools.export.region.valid());
        let single_image = || {
            self.workspace.board.idle()
                && self.workspace.board.selection.len() == 1
                && self.workspace.board.selection.iter().any(|id| {
                    self.workspace
                        .board
                        .scene
                        .get(*id)
                        .is_some_and(|i| i.blob().is_some())
                })
        };
        match action {
            ExportScene | ExportOriginals | ExportZip => {
                export_ready
                    && self.workspace.board.idle()
                    && !self.workspace.board.scene.is_empty()
            }
            ExportSelection => {
                export_ready
                    && self.workspace.board.idle()
                    && !self.workspace.board.selection.is_empty()
            }
            ExportOriginal => export_ready && single_image(),
            CopyImage | OpenOriginal => single_image(),
            // The system picker, where there is one (the desktop portal, macOS).
            PickScreen => self.shell.window.as_ref().is_some_and(|window| {
                moth_platform::screen_color::capabilities(window).interactive
            }),
            _ => self.workspace.board.idle() && !self.workspace.board.scene.is_empty(),
        }
    }
    /// Runs an action of this area; `None` when it finished without an intent.
    pub(super) fn run_export(
        &mut self,
        action: Action,
        _event_loop: &ActiveEventLoop,
    ) -> Option<Intent> {
        use Action::*;
        let items = Self::selected_items(&self.workspace.board, action);
        let intent = match action {
            Export => {
                self.workspace.board.notices.export_message = None;
                self.tools.export.open = true;
                return None;
            }
            SampleColor => {
                self.workspace.board.notices.export_message = None;
                self.tool = super::gesture::Tool::PickColor;
                self.tools.pickers.open = true;
                return None;
            }
            PickScreen => {
                self.pick_from_screen();
                return None;
            }
            CoordinatePicker => {
                self.workspace.board.notices.export_message = None;
                self.tool = super::gesture::Tool::PickInfo;
                self.tools.pickers.open = true;
                return None;
            }
            OpenOriginal => {
                let [item] = items.as_slice() else {
                    return None;
                };
                self.tools.viewer = Some(super::super::tools::ViewerNotice::new());
                Intent::Export(moth_platform::export::Target::Viewer(item.id))
            }
            CopyImage => {
                let [item] = items.as_slice() else {
                    return None;
                };
                Intent::Export(moth_platform::export::Target::Clipboard(item.id))
            }
            ExportScene | ExportSelection | ExportOriginal | ExportOriginals | ExportZip => {
                use super::super::tools::PendingExport;
                let pending = match action {
                    ExportOriginal => {
                        let [item] = items.as_slice() else {
                            return None;
                        };
                        PendingExport::Original(item.id, item.name.clone())
                    }
                    // Groups count here: a selected group brings the images inside it.
                    ExportOriginals => PendingExport::Originals(
                        self.workspace.board.selection.iter().copied().collect(),
                    ),
                    ExportZip => PendingExport::Archive(items.iter().map(|i| i.id).collect()),
                    _ => PendingExport::Scene {
                        selection: action == ExportSelection,
                        ids: if action == ExportSelection {
                            items.iter().map(|i| i.id).collect()
                        } else {
                            Vec::new()
                        },
                        region: self.tools.export.crop.then_some(self.tools.export.region),
                        side: self.tools.export.side,
                    },
                };
                self.choose_export_path(pending);
                return None;
            }
            _ => return None,
        };
        Some(intent)
    }
    /// Asks where to export with the system "Save as" dialog, on its own thread like
    /// the board save dialog; [`Self::picked_export_path`] sends the export once it answers.
    fn choose_export_path(&mut self, pending: super::super::tools::PendingExport) {
        if self.window.export_picker.is_some() {
            return;
        }
        let Some(window) = self.shell.window.clone() else {
            return;
        };
        self.workspace.board.notices.export_message = None;
        let title = self.workspace.board.title().replace(['/', '\\'], "-");
        let dialog = pending.dialog(&title).set_parent(window.as_ref());
        let (send, receive) = crossbeam_channel::bounded(1);
        let wake = self.shell.wake();
        match std::thread::Builder::new()
            .name("export-picker".into())
            .spawn(move || {
                let path =
                    std::panic::catch_unwind(move || dialog.save_file()).unwrap_or_else(|_| {
                        tracing::error!("native export dialog panicked");
                        None
                    });
                let _ = send.send(path);
                wake();
                drop(window);
            }) {
            Ok(_) => self.window.export_picker = Some((receive, pending)),
            Err(error) => self.workspace.board.notices.message = error.to_string().into(),
        }
    }
    /// The export dialog answered: export there, or do nothing when it was closed.
    pub(in crate::app) fn picked_export_path(&mut self, event_loop: &ActiveEventLoop) {
        let Some((receive, _)) = &self.window.export_picker else {
            return;
        };
        let path = match receive.try_recv() {
            Ok(path) => path,
            Err(crossbeam_channel::TryRecvError::Empty) => return,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                tracing::error!("export dialog disconnected before returning a result");
                None
            }
        };
        let Some((_, pending)) = self.window.export_picker.take() else {
            return;
        };
        if let Some(path) = path {
            self.apply(Intent::Export(pending.target(path)), event_loop);
        }
    }
}
