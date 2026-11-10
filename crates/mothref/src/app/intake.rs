// SPDX-License-Identifier: GPL-3.0-or-later
//! Owns image-burst completion, its undo group and post-import arrangement.
use super::*;
use workspace::Workspace;

pub(super) struct Added {
    pub ids: Vec<Uuid>,
    pub mode: super::arrange::Arrange,
    pub needs_fit: bool,
}

/// One image burst, from arriving files through its final arrangement.
#[derive(Default)]
pub(super) struct Burst {
    batching: bool,
    ids: Vec<Uuid>,
    queued: usize,
    files_received: bool,
    idle_since: Option<Instant>,
    prompt: Option<Vec<Uuid>>,
    added: Option<Added>,
}
impl Burst {
    pub(super) fn receiving_files(&mut self) {
        self.files_received = false;
    }
    pub(super) fn files_received(&mut self) {
        self.files_received = true;
    }
    pub(super) fn queue_file(&mut self) {
        self.queued += 1;
    }
    pub(super) fn queued(&self) -> usize {
        self.queued
    }
    pub(super) fn prompt(&self) -> Option<&Vec<Uuid>> {
        self.prompt.as_ref()
    }
    pub(super) fn added(&self) -> Option<&Added> {
        self.added.as_ref()
    }
    pub(super) fn dismiss_prompt(&mut self) {
        self.prompt = None;
    }
    /// Late arrivals still belong to the drop whose prompt is already on screen.
    pub(super) fn add(&mut self, id: Uuid) {
        match &mut self.prompt {
            Some(ids) => ids.push(id),
            None => self.ids.push(id),
        }
    }
    fn settled(&self, idle: bool, now: Instant) -> Result<(), Duration> {
        const QUIET: Duration = Duration::from_millis(300);
        if idle && self.files_received {
            return Ok(());
        }
        match self.idle_since {
            Some(since) if idle => match QUIET.checked_sub(now.saturating_duration_since(since)) {
                None | Some(Duration::ZERO) => Ok(()),
                Some(wait) => Err(wait),
            },
            _ => Err(QUIET),
        }
    }
}
impl Workspace {
    pub(super) fn finish_added(&mut self) -> bool {
        if self.board.intake.burst.added.is_some() {
            self.end_batch();
            if self.board.intake.burst.batching {
                return false;
            }
            self.board.intake.burst.added = None;
        }
        true
    }
    pub(super) fn begin_batch(&mut self) {
        if self.board.intake.loading || !self.finish_added() {
            return;
        }
        if !self.board.intake.burst.batching
            && self
                .board
                .boards()
                .requests
                .try_send(moth_platform::boards::Request::BeginBatch)
                .is_ok()
        {
            self.board.intake.burst.batching = true;
        }
    }
    fn end_batch(&mut self) {
        if self.board.intake.burst.batching
            && self
                .board
                .boards()
                .requests
                .try_send(moth_platform::boards::Request::EndBatch)
                .is_ok()
        {
            self.board.intake.burst.batching = false;
        }
    }
    /// Starting a paste or drop closes the previous toast's undo group first.
    pub(super) fn prepare_intake(&mut self) -> bool {
        if self.board.locked
            || self.board.intake.loading
            || self.board.file.discarding
            || !self.finish_added()
        {
            return false;
        }
        self.board.intake.burst.dismiss_prompt();
        true
    }
    pub(super) fn paste(&mut self) {
        if !self.prepare_intake() {
            return;
        }
        match self
            .board
            .clipboard()
            .requests
            .try_send(moth_platform::ClipboardRequest::Paste)
        {
            Ok(()) => self.board.intake.pasting += 1,
            Err(_) => self.board.notices.message = moth_ui::Text::ClipboardBusy.into(),
        }
    }

    /// Layouts keep the insertion undo group open; refused requests retain its toast.
    pub(super) fn arrange_added(
        &mut self,
        commands: Vec<Command>,
        ids: Vec<Uuid>,
        mode: arrange::Arrange,
    ) -> bool {
        let previous = self.board.intake.burst.added.take();
        if !commands.is_empty() {
            let pending = self.board.intake.in_flight;
            self.persist_batch(commands.into_iter().map(|c| (c, None)).collect());
            if self.board.intake.in_flight == pending {
                self.board.intake.burst.added = previous;
                return false;
            }
        }
        self.board.intake.burst.prompt = None;
        self.board.intake.burst.added = Some(Added {
            ids,
            mode,
            needs_fit: true,
        });
        true
    }
    /// The UI and worker drain share readiness, including a native drop end marker.
    pub(super) fn intake_settled(&self) -> Result<(), Duration> {
        self.board
            .intake
            .burst
            .settled(self.board.idle() && !self.receiving_drop, Instant::now())
    }
    pub(super) fn can_complete_import(&self) -> bool {
        let requests = &self.board.boards().requests;
        // A completion can end the previous undo batch, begin a new one and insert.
        requests
            .capacity()
            .is_some_and(|capacity| requests.len() + 3 <= capacity)
    }
    pub(super) fn settle_intake(&mut self) {
        if !self.board.idle() || self.receiving_drop {
            self.board.intake.burst.idle_since = None;
            return;
        }
        self.board.intake.burst.queued = 0;
        self.board
            .intake
            .burst
            .idle_since
            .get_or_insert_with(Instant::now);
        if let Some(added) = &mut self.board.intake.burst.added {
            if added.needs_fit {
                if added.ids.len() > 1 {
                    let ids = added.ids.iter().copied().collect();
                    self.board.view.fit_selection(&self.board.scene, &ids);
                }
                added.needs_fit = false;
            }
        } else if self.board.intake.burst.batching && self.board.intake.burst.prompt.is_none() {
            if self.board.intake.burst.ids.is_empty() {
                self.end_batch();
            } else {
                self.board.intake.burst.prompt =
                    Some(std::mem::take(&mut self.board.intake.burst.ids));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrange_prompt_waits_for_the_whole_drop() {
        let now = Instant::now();
        let mut burst = Burst {
            prompt: Some((0..400).map(|_| Uuid::now_v7()).collect()),
            ..Burst::default()
        };
        for _ in 0..614 {
            burst.add(Uuid::now_v7());
        }
        assert_eq!(burst.prompt().unwrap().len(), 1014);
        assert!(burst.ids.is_empty(), "late images join the existing prompt");
        assert_eq!(burst.settled(true, now), Err(Duration::from_millis(300)));
        burst.idle_since = Some(now);
        assert_eq!(
            burst.settled(true, now + Duration::from_millis(299)),
            Err(Duration::from_millis(1)),
        );
        assert!(
            burst
                .settled(true, now + Duration::from_millis(300))
                .is_ok()
        );
        assert!(burst.settled(false, now + Duration::from_secs(1)).is_err());
        burst.idle_since = None;
        burst.files_received();
        assert!(
            burst.settled(false, now).is_err(),
            "workers still own images"
        );
        assert!(
            burst.settled(true, now).is_ok(),
            "a complete list needs no timer"
        );
        burst.receiving_files();
        assert!(
            burst.settled(true, now).is_err(),
            "the next list is incomplete"
        );
    }
}
