// SPDX-License-Identifier: GPL-3.0-or-later
//! Replaceable viewport work shares a bounded queue with non-cancellable imports.
use crate::Request;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

const CAPACITY: usize = 8;
#[derive(Default)]
struct State {
    queued: VecDeque<Request>,
    wanted: Vec<Request>,
    running: Vec<(Request, Arc<AtomicBool>)>,
    finished: Vec<Request>,
    closed: bool,
}
#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    ready: Condvar,
}
impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
/// Imports are never cancelled; viewport requests are replaced as the camera moves.
#[derive(Default)]
pub struct Requests(Arc<Shared>);
impl Drop for Requests {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        state.closed = true;
        for (_, cancel) in &state.running {
            cancel.store(true, Ordering::Relaxed);
        }
        self.0.ready.notify_all();
    }
}
impl Requests {
    /// Keeps existing import backpressure without letting preview work discard originals.
    pub fn try_send(
        &self,
        request: Request,
    ) -> Result<(), crossbeam_channel::TrySendError<Request>> {
        let mut state = self.0.lock();
        if state.closed {
            return Err(crossbeam_channel::TrySendError::Disconnected(request));
        }
        if state.queued.len() == CAPACITY {
            return Err(crossbeam_channel::TrySendError::Full(request));
        }
        state.queued.push_back(request);
        self.0.ready.notify_one();
        Ok(())
    }
    /// The caller supplies highest priority first. Only eight requests are retained.
    pub fn schedule(&self, requests: impl IntoIterator<Item = Request>) {
        let mut state = self.0.lock();
        let mut wanted = Vec::with_capacity(CAPACITY);
        for request in requests {
            if !wanted.contains(&request) {
                wanted.push(request);
            }
            if wanted.len() == CAPACITY {
                break;
            }
        }
        state.queued.retain(|r| r.blob.is_none());
        state.finished.retain(|r| wanted.contains(r));
        for (request, cancel) in &state.running {
            if request.blob.is_some()
                && !state
                    .running
                    .iter()
                    .any(|(r, c)| Arc::ptr_eq(c, cancel) && wanted.contains(r))
            {
                cancel.store(true, Ordering::Relaxed);
            }
        }
        for request in &wanted {
            if state.queued.len() == CAPACITY {
                break;
            }
            if !state.finished.contains(request)
                && !state
                    .running
                    .iter()
                    .any(|(r, cancel)| r == request && !cancel.load(Ordering::Relaxed))
            {
                state.queued.push_back(*request);
            }
        }
        state.wanted = wanted;
        self.0.ready.notify_all();
    }
    /// A completion can become stale while waiting in the bounded result channel.
    pub fn current(&self, request: Request) -> bool {
        request.blob.is_none() || self.0.lock().wanted.contains(&request)
    }
    /// Releases the result slot after UI handling; failed GPU allocations can then retry.
    pub fn acknowledge(&self, request: Request) {
        self.0.lock().finished.retain(|r| *r != request);
    }
    pub(crate) fn worker(&self) -> Worker {
        Worker(self.0.clone())
    }
}
pub(crate) struct Worker(Arc<Shared>);
impl Worker {
    pub fn next_batch(&self) -> Option<(Vec<Request>, Arc<AtomicBool>)> {
        let mut state = self.0.lock();
        loop {
            if state.closed {
                return None;
            }
            let ready = state.queued.iter().position(|request| {
                request.tile.is_none()
                    || !state.running.iter().any(|(r, _)| {
                        r.tile.is_some() && r.blob == request.blob && r.handle == request.handle
                    })
            });
            if let Some(index) = ready {
                let request = state
                    .queued
                    .remove(index)
                    .expect("queued position exists under the lock");
                let mut batch = vec![request];
                if request.tile.is_some() {
                    state.queued.retain(|r| {
                        let same = r.tile.map(|tile| tile.level)
                            == request.tile.map(|tile| tile.level)
                            && r.blob == request.blob
                            && r.handle == request.handle
                            && r.format == request.format;
                        if same {
                            batch.push(*r);
                        }
                        !same
                    });
                }
                let cancel = Arc::new(AtomicBool::new(false));
                state
                    .running
                    .extend(batch.iter().map(|r| (*r, cancel.clone())));
                return Some((batch, cancel));
            }
            state = self
                .0
                .ready
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
    #[cfg(test)]
    pub fn next(&self) -> Option<(Request, Arc<AtomicBool>)> {
        let mut state = self.0.lock();
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.queued.pop_front() {
                let cancel = Arc::new(AtomicBool::new(false));
                state.running.push((request, cancel.clone()));
                return Some((request, cancel));
            }
            state = self
                .0
                .ready
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
    pub fn finish(&self, request: Request, cancel: &Arc<AtomicBool>) -> bool {
        let mut state = self.0.lock();
        state
            .running
            .retain(|(r, c)| *r != request || !Arc::ptr_eq(c, cancel));
        self.0.ready.notify_all();
        let current = !cancel.load(Ordering::Relaxed)
            && !state.closed
            && (request.blob.is_none() || state.wanted.contains(&request));
        if current && request.blob.is_some() && !state.finished.contains(&request) {
            state.finished.push(request);
        }
        current
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request(side: u32, blob: bool) -> Request {
        Request {
            tile: None,
            handle: moth_format::Handle(1),
            side,
            format: crate::ImageFormat::Rgba8,
            blob: blob.then(|| blake3::hash(b"image")),
        }
    }
    #[test]
    fn partial_batch_cancellation_keeps_needed_tiles_running() {
        let requests = Requests::default();
        let worker = requests.worker();
        let a = Request {
            tile: Some(crate::tiles::Tile {
                level: 0,
                x: 0,
                y: 0,
            }),
            ..request(256, true)
        };
        let b = Request {
            tile: Some(crate::tiles::Tile {
                level: 0,
                x: 1,
                y: 0,
            }),
            ..a
        };
        requests.schedule([a, b]);
        let (batch, cancel) = worker.next_batch().unwrap();
        assert_eq!(batch, [a, b]);
        requests.schedule([b]);
        assert!(!cancel.load(Ordering::Relaxed));
        assert!(!worker.finish(a, &cancel));
        assert!(worker.finish(b, &cancel));
        requests.acknowledge(b);
        requests.schedule([a, b]);
        let (_, cancel) = worker.next_batch().unwrap();
        requests.schedule([]);
        assert!(cancel.load(Ordering::Relaxed));
    }
    #[test]
    fn replacing_work_cancels_old_levels_but_keeps_imports() {
        let requests = Requests::default();
        let worker = requests.worker();
        requests.schedule([request(16, true)]);
        let (old, cancel) = worker.next().unwrap();
        requests.try_send(request(256, false)).unwrap();
        requests.schedule([request(2048, true)]);
        assert!(cancel.load(Ordering::Relaxed));
        assert!(!worker.finish(old, &cancel));
        let (import, cancel) = worker.next().unwrap();
        assert!(import.blob.is_none());
        assert!(worker.finish(import, &cancel));
        let (new, cancel) = worker.next().unwrap();
        assert_eq!(new.side, 2048);
        assert!(worker.finish(new, &cancel));
        requests.schedule([new]);
        assert!(
            requests.0.lock().queued.is_empty(),
            "a completion waiting for upload is not duplicated"
        );
        requests.acknowledge(new);
        requests.schedule([new]);
        assert_eq!(
            requests.0.lock().queued.front(),
            Some(&new),
            "an upload refused by the GPU budget can retry"
        );
        requests.schedule([]);
        assert!(!requests.current(new));
    }
    #[test]
    fn queue_is_bounded_and_preserves_priority() {
        let requests = Requests::default();
        requests.schedule((1..100).map(|n| request(n, true)));
        assert_eq!(requests.0.lock().queued.len(), CAPACITY);
        let worker = requests.worker();
        for n in 1..=8 {
            let (request, cancel) = worker.next().unwrap();
            assert_eq!(request.side, n);
            assert!(worker.finish(request, &cancel));
        }
    }
}
