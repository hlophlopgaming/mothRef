// SPDX-License-Identifier: GPL-3.0-or-later
//! Owns a board's scene, persistence identity and bounded background loading work.
use super::*;

/// Owns document data and workers so tab switches cannot exchange persistence identities.
pub(super) struct Document {
    pub(super) storage: Arc<moth_platform::DesktopStorage>,
    pub(super) decode: moth_io::DecodePool,
    pub(super) clipboard: moth_platform::ClipboardWorker,
    pub(super) scene: Scene,
    pub(super) boards: moth_platform::boards::Worker,
    pub(super) in_flight: usize,
    pub(super) pasting: usize,
    pub(super) path: String,
    pub(super) dirty: bool,
    pub(super) close_after_save: bool,
    pub(super) sources: HashMap<BlobId, Handle>,
    pub(super) pending: HashMap<Handle, Pending>,
    pub(super) failed: HashSet<Handle>,
    pub(super) waiting: Vec<Handle>,
    pub(super) discarding: bool,
    pub(super) discarded: bool,
    pub(super) batching: bool,
    pub(super) burst: Vec<Uuid>,
    pub(super) arrange_prompt: Option<Vec<Uuid>>,
}
impl Document {
    pub(super) fn with_storage(
        storage: Arc<moth_platform::DesktopStorage>,
        wake: Arc<dyn Fn() + Send + Sync>,
        discover: bool,
    ) -> anyhow::Result<Self> {
        let decode = moth_io::DecodePool::new(storage.clone(), wake.clone())?;
        let clipboard = moth_platform::ClipboardWorker::new(storage.clone(), wake.clone())?;
        let boards = if discover {
            moth_platform::boards::Worker::new(storage.clone(), wake)?
        } else {
            moth_platform::boards::Worker::new_tab(storage.clone(), wake)?
        };
        Ok(Self {
            storage,
            decode,
            clipboard,
            scene: Scene::default(),
            boards,
            in_flight: 0,
            pasting: 0,
            path: String::new(),
            dirty: false,
            close_after_save: false,
            sources: HashMap::new(),
            pending: HashMap::new(),
            failed: HashSet::new(),
            waiting: Vec::new(),
            discarding: false,
            discarded: false,
            batching: false,
            burst: Vec::new(),
            arrange_prompt: None,
        })
    }
    pub(super) fn idle(&self) -> bool {
        self.pasting == 0
            && self.in_flight == 0
            && self.pending.is_empty()
            && self.waiting.is_empty()
            && !self.discarding
    }
    pub(super) fn title(&self) -> &str {
        std::path::Path::new(&self.path)
            .file_stem()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .unwrap_or(moth_ui::text(moth_ui::Text::Untitled))
    }
    /// Opens an undo group for a burst of asynchronous inserts (drop/paste).
    pub(super) fn begin_batch(&mut self) {
        if !self.batching
            && self
                .boards
                .requests
                .try_send(moth_platform::boards::Request::BeginBatch)
                .is_ok()
        {
            self.batching = true;
        }
    }
    /// Closes the undo group once the burst has fully drained to an idle board.
    pub(super) fn end_batch(&mut self) {
        if self.batching
            && self
                .boards
                .requests
                .try_send(moth_platform::boards::Request::EndBatch)
                .is_ok()
        {
            self.batching = false;
        }
    }
    /// Pushes queued image handles onto the decode pool, bounded by its capacity.
    pub(super) fn pump(&mut self, format: moth_io::ImageFormat) {
        while let Some(&handle) = self.waiting.last() {
            match self.decode.requests.try_send(moth_io::Request {
                handle,
                side: 256,
                format,
            }) {
                Ok(()) => {
                    self.waiting.pop();
                    self.pending.insert(handle, Pending::Import);
                }
                Err(_) => break,
            }
        }
    }
}
