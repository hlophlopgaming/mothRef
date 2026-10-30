// SPDX-License-Identifier: GPL-3.0-or-later
//! Export, the image pickers and opening originals elsewhere.
use super::*;
impl App {
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn export_enabled(&self, action: Action) -> bool {
        use Action::*;
        let export_ready =
            !self.tools.path.is_empty() && (!self.tools.crop || self.tools.region.valid());
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
                self.tools.export_open = true;
                return None;
            }
            SampleColor => {
                self.workspace.board.notices.export_message = None;
                self.tool = super::gesture::Tool::PickColor;
                self.tools.picker_open = true;
                return None;
            }
            CoordinatePicker => {
                self.workspace.board.notices.export_message = None;
                self.tool = super::gesture::Tool::PickInfo;
                self.tools.picker_open = true;
                return None;
            }
            OpenOriginal | CopyImage | ExportScene | ExportSelection | ExportOriginal
            | ExportOriginals | ExportZip => {
                use moth_platform::export::Target;
                let path = std::path::PathBuf::from(&self.tools.path);
                let target = match action {
                    OpenOriginal => {
                        self.tools.viewer_open = true;
                        let [item] = items.as_slice() else {
                            return None;
                        };
                        Target::Viewer(item.id)
                    }
                    CopyImage => {
                        let [item] = items.as_slice() else {
                            return None;
                        };
                        Target::Clipboard(item.id)
                    }
                    ExportOriginal => {
                        let [item] = items.as_slice() else {
                            return None;
                        };
                        Target::Original { path, id: item.id }
                    }
                    ExportOriginals => Target::Originals(path),
                    ExportZip => Target::Archive {
                        path,
                        ids: items.iter().map(|i| i.id).collect(),
                    },
                    _ => Target::Scene {
                        path,
                        ids: if action == ExportSelection {
                            items.iter().map(|i| i.id).collect()
                        } else {
                            Vec::new()
                        },
                        region: self.tools.crop.then_some(self.tools.region),
                        side: self.tools.side,
                    },
                };
                Intent::Export(target)
            }
            _ => return None,
        };
        Some(intent)
    }
}
