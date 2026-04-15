/// Tracks datagram send success/failure counts for bitrate adaptation.
pub(super) struct DatagramStats {
    successes: std::sync::atomic::AtomicU64,
    failures: std::sync::atomic::AtomicU64,
}

impl DatagramStats {
    pub(super) fn new() -> Self {
        Self {
            successes: std::sync::atomic::AtomicU64::new(0),
            failures: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub(super) fn record_success(&self) {
        self.successes
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub(super) fn record_failure(&self) {
        self.failures
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Take and reset the counters. Returns `(successes, failures)`.
    pub(super) fn take_counts(&self) -> (u64, u64) {
        let successes = self.successes.swap(0, std::sync::atomic::Ordering::Relaxed);
        let failures = self.failures.swap(0, std::sync::atomic::Ordering::Relaxed);
        (successes, failures)
    }
}

/// Compute an adapted bitrate given the current bitrate and datagram
/// success/failure counts observed during the last sample window.
///
/// Rules:
/// - >10% failure   → reduce by 20%
/// -  2–10% failure → reduce by 5%
/// -  0% failure    → increase by 5%
/// - otherwise      → no change
///
/// The result is clamped to `[200 kbps, 8 Mbps]`.
pub(super) fn compute_adapted_bitrate(current_bps: u32, successes: u64, failures: u64) -> u32 {
    let total = successes + failures;
    if total == 0 {
        return current_bps;
    }

    let failure_rate = failures as f32 / total as f32;
    let new = if failure_rate > 0.10 {
        (current_bps as f32 * 0.8) as u32
    } else if failure_rate > 0.02 {
        (current_bps as f32 * 0.95) as u32
    } else if failure_rate < 0.01 && failures == 0 {
        (current_bps as f32 * 1.05) as u32
    } else {
        current_bps
    };

    new.clamp(200_000, 8_000_000)
}
