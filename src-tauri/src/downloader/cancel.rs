//! Per-operation cancellation via a generation counter.
//!
//! Each operation snapshots the current generation at start and checks
//! `is_cancelled(snapshot)` between units of work. `cancel()` bumps the
//! generation, which invalidates every in-flight operation's snapshot —
//! no shared reset needed, and cancelling one operation never affects
//! another operation that started after the bump.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone)]
pub struct Cancellation {
    gen: Arc<AtomicU64>,
}

impl Cancellation {
    pub fn new() -> Self {
        Self { gen: Arc::new(AtomicU64::new(0)) }
    }

    /// Clone the handle (shares the same generation counter).
    pub fn share(&self) -> Cancellation {
        Cancellation { gen: Arc::clone(&self.gen) }
    }

    /// Snapshot the current generation at operation start.
    pub fn snapshot(&self) -> u64 {
        self.gen.load(Ordering::Relaxed)
    }

    /// Cancel every operation that snapshotted before this call.
    pub fn cancel(&self) {
        self.gen.fetch_add(1, Ordering::Relaxed);
    }

    /// True if the operation holding `snapshot` should stop.
    pub fn is_cancelled(&self, snapshot: u64) -> bool {
        self.gen.load(Ordering::Relaxed) != snapshot
    }
}

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_unchanged_until_cancel() {
        let c = Cancellation::new();
        let s = c.snapshot();
        assert!(!c.is_cancelled(s));
        c.cancel();
        assert!(c.is_cancelled(s));
        // A new operation after the bump is not cancelled
        let s2 = c.snapshot();
        assert!(!c.is_cancelled(s2));
    }

    #[test]
    fn test_share_sees_cancels() {
        let c = Cancellation::new();
        let shared = c.share();
        let s = c.snapshot();
        shared.cancel();
        assert!(c.is_cancelled(s));
    }
}
