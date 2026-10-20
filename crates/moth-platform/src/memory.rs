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

/// Share of available RAM the in-memory performance modes may keep as decoded pyramids.
pub const LOD_RAM_FRACTION: f64 = 0.25;
/// Absolute ceiling for decoded pyramids kept in RAM.
pub const LOD_RAM_LIMIT: u64 = 4 * 1024 * 1024 * 1024;
/// Allowance when available RAM is unknown.
pub const LOD_RAM_FALLBACK: u64 = 512 * 1024 * 1024;

/// RAM budget of the CPU-side LOD cache; zero unless an in-memory mode is selected.
/// The cache's own bytes count as available, so its growth does not shrink its budget.
pub fn lod_cache_budget(available: Option<u64>, own: u64, in_memory: bool) -> u64 {
    if !in_memory {
        return 0;
    }
    available
        .map_or(LOD_RAM_FALLBACK, |bytes| {
            (bytes.saturating_add(own) as f64 * LOD_RAM_FRACTION) as u64
        })
        .min(LOD_RAM_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lod_budget_follows_mode_and_ram() {
        let gib = 1024 * 1024 * 1024;
        assert_eq!(lod_cache_budget(Some(8 * gib), 0, false), 0);
        assert_eq!(lod_cache_budget(Some(8 * gib), 0, true), 2 * gib);
        assert_eq!(lod_cache_budget(Some(7 * gib), gib, true), 2 * gib);
        assert_eq!(lod_cache_budget(Some(64 * gib), 0, true), LOD_RAM_LIMIT);
        assert_eq!(lod_cache_budget(None, 0, true), LOD_RAM_FALLBACK);
        assert_eq!(
            lod_cache_budget(Some(u64::MAX), u64::MAX, true),
            LOD_RAM_LIMIT
        );
    }
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
