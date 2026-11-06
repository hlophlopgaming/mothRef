// SPDX-License-Identifier: GPL-3.0-or-later
//! Admission for the few decodes that hold a whole frame (WebP, progressive JPEG).
//!
//! Row-streamed formats need a few rows at a time, but a 100-megapixel WebP needs
//! 400 MiB at once. Without a gate every decode worker could hold one, so memory would
//! grow with the core count. Large frames share one budget; a single frame is always
//! admitted so no image is refused for being alone over the budget.
use std::sync::{Condvar, Mutex, PoisonError};

/// Frames below this run freely: they are what the old 32-megapixel limit allowed per worker.
const UNGATED: u64 = 64 * 1024 * 1024;
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
}
