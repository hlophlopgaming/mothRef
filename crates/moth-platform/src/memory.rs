// SPDX-License-Identifier: GPL-3.0-or-later
//! RAM queries never block a frame or keep an idle application repainting.
use crate::Platform;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// One queued query and one result bound sampling independently of board size.
pub struct Monitor {
    requests: crossbeam_channel::Sender<()>,
    results: crossbeam_channel::Receiver<Option<u64>>,
    last: Option<Instant>,
    available: Option<u64>,
}
impl Monitor {
    /// The platform call, including any filesystem access, runs on this worker.
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<Self> {
        let (requests, rx) = crossbeam_channel::bounded(1);
        let (tx, results) = crossbeam_channel::bounded(1);
        std::thread::Builder::new()
            .name("memory".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    if tx.send(crate::Desktop.available_memory()).is_err() {
                        break;
                    }
                    wake();
                }
            })?;
        Ok(Self {
            requests,
            results,
            last: None,
            available: None,
        })
    }
    /// Returns the latest sample and requests updates at most once per two active seconds.
    pub fn available(&mut self) -> Option<u64> {
        while let Ok(sample) = self.results.try_recv() {
            self.available = sample;
        }
        if self
            .last
            .is_none_or(|last| last.elapsed() >= Duration::from_secs(2))
            && self.requests.try_send(()).is_ok()
        {
            self.last = Some(Instant::now());
        }
        self.available
    }
}

/// Reclaimable image pages count toward our allowance, avoiding feedback as the cache grows.
pub fn texture_ceiling(available: Option<u64>, allocated: u64, limit: u64, percent: f32) -> u64 {
    let fraction = if percent.is_finite() {
        f64::from(percent.clamp(1., 50.)) / 100.
    } else {
        0.125
    };
    available
        .map_or(limit, |bytes| {
            (bytes.saturating_add(allocated) as f64 * fraction) as u64
        })
        .min(limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allowance_tracks_ram_without_oscillating_with_own_allocations() {
        let mib = 1024 * 1024;
        assert_eq!(
            texture_ceiling(Some(512 * mib), 0, 512 * mib, 12.5),
            64 * mib
        );
        assert_eq!(
            texture_ceiling(Some(512 * mib), 0, 512 * mib, 25.),
            128 * mib
        );
        assert_eq!(
            texture_ceiling(Some(512 * mib), 0, 512 * mib, f32::NAN),
            64 * mib
        );
        assert_eq!(
            texture_ceiling(Some(480 * mib), 32 * mib, 512 * mib, 12.5),
            64 * mib
        );
        assert_eq!(
            texture_ceiling(Some(8 * 1024 * mib), 0, 96 * mib, 12.5),
            96 * mib
        );
        assert_eq!(texture_ceiling(Some(0), 0, 96 * mib, 12.5), 0);
        assert_eq!(texture_ceiling(None, 0, 96 * mib, 12.5), 96 * mib);
        assert_eq!(
            texture_ceiling(Some(u64::MAX), u64::MAX, 512 * mib, 12.5),
            512 * mib
        );
    }
}
