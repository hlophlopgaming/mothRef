// SPDX-License-Identifier: GPL-3.0-or-later
//! Admission for the few decodes that hold a whole frame (WebP, progressive JPEG).
//!
//! Row-streamed formats need a few rows at a time, but a 100-megapixel WebP needs
//! 400 MiB at once. Without a gate every decode worker could hold one, so memory would
//! grow with the core count. Large frames share one budget; a single frame is always
//! admitted so no image is refused for being alone over the budget.
use std::sync::{Arc, Condvar, Mutex, PoisonError};

/// Frames below this run freely: they are what the old 32-megapixel limit allowed per worker.
const UNGATED: u64 = 64 * 1024 * 1024;
/// Whole frames at least this large are kept for the next tile batch (see [`shared`]);
/// smaller ones decode quickly enough to repeat.
pub(crate) const SHARED_FROM: u64 = UNGATED;
/// Combined transient bytes of large frames decoded at the same time.
const BUDGET: u64 = crate::MAX_COEFFICIENT_BYTES;

static IN_USE: Mutex<u64> = Mutex::new(0);
static RELEASED: Condvar = Condvar::new();

/// Returns the admitted share to the budget when the decode ends, including by panic.
pub(crate) struct Admission(u64);

impl Drop for Admission {
    fn drop(&mut self) {
        let mut in_use = IN_USE.lock().unwrap_or_else(PoisonError::into_inner);
        *in_use = in_use.saturating_sub(self.0);
        RELEASED.notify_all();
    }
}

/// Waits on a decode worker (never the main thread) until `bytes` fit the shared budget.
pub(crate) fn admit(bytes: u64) -> Option<Admission> {
    if bytes < UNGATED {
        return None;
    }
    let bytes = bytes.min(BUDGET);
    let mut in_use = IN_USE.lock().unwrap_or_else(PoisonError::into_inner);
    while *in_use > 0 && *in_use + bytes > BUDGET {
        in_use = RELEASED
            .wait(in_use)
            .unwrap_or_else(PoisonError::into_inner);
    }
    *in_use += bytes;
    Some(Admission(bytes))
}

/// The one large whole frame kept between tile batches (WebP has no row decoder).
/// Viewport batches of the same image would otherwise each decode the full frame,
/// up to one per decode worker at once. Budget: one frame, released when the image's
/// tiles are baked (they are then read from the pack) or another large frame replaces it.
struct Memo {
    slot: Option<(blake3::Hash, Arc<image::RgbaImage>)>,
    decoding: Vec<blake3::Hash>,
}
static MEMO: Mutex<Memo> = Mutex::new(Memo {
    slot: None,
    decoding: Vec::new(),
});
static DECODED: Condvar = Condvar::new();

/// The decoded frame of `hash`: shared when another worker has it or is decoding it,
/// otherwise decoded here by `decode` (without holding the lock while it runs).
pub(crate) fn shared(
    hash: blake3::Hash,
    decode: impl FnOnce() -> Result<image::RgbaImage, crate::DecodeError>,
) -> Result<Arc<image::RgbaImage>, crate::DecodeError> {
    let mut memo = MEMO.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
        if let Some((held, frame)) = &memo.slot
            && *held == hash
        {
            return Ok(frame.clone());
        }
        if !memo.decoding.contains(&hash) {
            break;
        }
        memo = DECODED.wait(memo).unwrap_or_else(PoisonError::into_inner);
    }
    memo.decoding.push(hash);
    drop(memo);
    let result = decode().map(Arc::new);
    let mut memo = MEMO.lock().unwrap_or_else(PoisonError::into_inner);
    memo.decoding.retain(|other| *other != hash);
    if let Ok(frame) = &result {
        memo.slot = Some((hash, frame.clone()));
    }
    DECODED.notify_all();
    result
}

/// Drops the kept frame of `hash`: its tiles are baked and come from the pack now.
pub(crate) fn forget(hash: blake3::Hash) {
    let mut memo = MEMO.lock().unwrap_or_else(PoisonError::into_inner);
    if memo.slot.as_ref().is_some_and(|(held, _)| *held == hash) {
        memo.slot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_frames_skip_the_gate_and_large_ones_return_their_share() {
        assert!(admit(UNGATED - 1).is_none());
        let first = admit(BUDGET).expect("a lone frame is always admitted");
        drop(first);
        let again = admit(BUDGET);
        assert!(again.is_some(), "the share came back on drop");
    }
    #[test]
    fn a_shared_frame_is_decoded_once_and_forgotten_after_its_bake() {
        let hash = blake3::hash(b"frame memo test");
        let decodes = std::sync::atomic::AtomicUsize::new(0);
        let decode = || {
            decodes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(image::RgbaImage::new(2, 1))
        };
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| shared(hash, decode).unwrap());
            }
        });
        assert_eq!(decodes.load(std::sync::atomic::Ordering::Relaxed), 1);
        forget(hash);
        shared(hash, decode).unwrap();
        assert_eq!(decodes.load(std::sync::atomic::Ordering::Relaxed), 2);
        forget(hash);
    }
}
