// SPDX-License-Identifier: GPL-3.0-or-later
//! A tab owns its document and view; switching tabs never swaps persistence identities.
use super::*;
use super::{document::Document, view::View};
pub(super) const MAX_TABS: usize = 4;

/// Keeps one document paired with its view for the entire lifetime of a tab.
pub(super) struct Session {
    pub(super) ui_id: Uuid,
    pub(super) document: Document,
    pub(super) view: View,
}
impl Session {
    pub(super) fn new(wake: Arc<dyn Fn() + Send + Sync>, discover: bool) -> anyhow::Result<Self> {
        Self::with_storage(
            Arc::new(moth_platform::DesktopStorage::default()),
            wake,
            discover,
        )
    }
    fn with_storage(
        storage: Arc<moth_platform::DesktopStorage>,
        wake: Arc<dyn Fn() + Send + Sync>,
        discover: bool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            ui_id: Uuid::now_v7(),
            document: Document::with_storage(storage, wake, discover)?,
            view: View::default(),
        })
    }
    pub(super) fn fit(&mut self) {
        self.view.fit(&self.document.scene);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parked_tabs_keep_separate_journals_and_resume_their_own_scene() {
        let root = std::env::temp_dir().join(format!("mothref-tabs-{}", Uuid::now_v7()));
        let mut first = Session::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();
        let mut second = Session::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();
        let (source, image) = first
            .document
            .storage
            .stage_preview(1, 1, &[12, 34, 56, 255])
            .unwrap();
        let item = Item::image(
            image.hash,
            Rect {
                x: 10.,
                y: 20.,
                width: 30.,
                height: 40.,
            },
            None,
        );
        let id = item.id;
        first
            .document
            .boards
            .requests
            .send(moth_platform::boards::Request::Apply(
                Command::Insert(item),
                Some(source),
            ))
            .unwrap();
        match first
            .document
            .boards
            .results
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
        {
            moth_platform::boards::Completion::Applied(command, _) => {
                first.document.scene.apply(command).unwrap();
            }
            _ => panic!("insert must be acknowledged after journaling"),
        }
        assert!(second.document.scene.is_empty());
        first.fit();
        assert!(first.view.needs_fit);
        assert!(first.view.camera.origin.iter().all(|v| v.is_finite()));
        first.view.viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300., 200.));
        first.fit();
        assert!(!first.view.needs_fit);
        let center = first.view.camera.screen([25., 40.]);
        assert!((center[0] - 150.).abs() < 0.01 && (center[1] - 100.).abs() < 0.01);
        std::mem::swap(&mut first, &mut second);
        assert!(first.document.scene.is_empty());
        assert!(second.document.scene.get(id).is_some());
        second
            .document
            .boards
            .requests
            .send(moth_platform::boards::Request::Undo)
            .unwrap();
        match second
            .document
            .boards
            .results
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
        {
            moth_platform::boards::Completion::Applied(command, _) => {
                second.document.scene.apply(command).unwrap();
            }
            _ => panic!("undo must target the parked tab's journal"),
        }
        assert!(second.document.scene.is_empty());
        assert!(first.document.scene.is_empty());
        first.document.boards.finish().unwrap();
        second.document.boards.finish().unwrap();
        // Decode/clipboard handles keep no files open after their owning sessions are dropped.
        std::fs::remove_dir_all(root).unwrap();
    }
}
