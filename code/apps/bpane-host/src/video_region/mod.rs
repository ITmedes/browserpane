//! Editable hint tracking and H.264 video region management.
//!
//! These structs encapsulate the mutable state that tracks CDP editable
//! regions and FFmpeg capture region commit/jitter/h264 toggle logic.

#[cfg(test)]
mod tests;

use std::time::Instant;

use crate::capture::ffmpeg::CaptureRegion;

// ── Editable hint state ─────────────────────────────────────────────

/// Tracks the last CDP editable-region hint and key-input timestamps
/// to provide a short QOI-boost window around focused text fields.
pub struct EditableHintState {
    pub last_hint: Option<CaptureRegion>,
    pub last_hint_at: Option<Instant>,
    pub last_key_input_at: Option<Instant>,
}

impl EditableHintState {
    pub fn new() -> Self {
        Self {
            last_hint: None,
            last_hint_at: None,
            last_key_input_at: None,
        }
    }

    pub fn on_key_input(&mut self, now: Instant) {
        self.last_key_input_at = Some(now);
    }

    /// Update hint state from this frame's CDP editable region.
    pub fn update(&mut self, cdp_hint: Option<CaptureRegion>, now: Instant, hold_ms: u64) {
        if let Some(region) = cdp_hint {
            self.last_hint = Some(region);
            self.last_hint_at = Some(now);
        } else if self
            .last_hint_at
            .map(|ts| now.duration_since(ts) > std::time::Duration::from_millis(hold_ms))
            .unwrap_or(false)
        {
            self.last_hint = None;
            self.last_hint_at = None;
        }
    }

    /// Whether we're in the key-input QOI boost window.
    pub fn key_input_qoi_boost(&self, now: Instant, boost_ms: u64) -> bool {
        self.last_key_input_at
            .map(|ts| now.duration_since(ts) <= std::time::Duration::from_millis(boost_ms))
            .unwrap_or(false)
    }

    /// Compute the active editable QOI region, falling back to the held
    /// hint during the key-input boost window.
    pub fn qoi_region(
        &self,
        cdp_hint: Option<CaptureRegion>,
        now: Instant,
        hold_ms: u64,
        boost_ms: u64,
    ) -> Option<CaptureRegion> {
        cdp_hint.or_else(|| {
            if self.key_input_qoi_boost(now, boost_ms)
                && self
                    .last_hint_at
                    .map(|ts| now.duration_since(ts) <= std::time::Duration::from_millis(hold_ms))
                    .unwrap_or(false)
            {
                self.last_hint
            } else {
                None
            }
        })
    }
}

// ── Region commit / jitter filter ───────────────────────────────────

/// Manages the FFmpeg capture region with jitter filtering and
/// stable-frame commitment.
pub struct RegionCommitter {
    pub active: Option<CaptureRegion>,
    pending: Option<CaptureRegion>,
    streak: u8,
    pub last_reconfig_at: Instant,
}

impl RegionCommitter {
    pub fn new() -> Self {
        Self {
            active: None,
            pending: None,
            streak: 0,
            last_reconfig_at: Instant::now(),
        }
    }

    /// Feed the next candidate region and return the committed region
    /// (which may lag behind the candidate to filter jitter).
    pub fn commit(
        &mut self,
        next: Option<CaptureRegion>,
        stable_frames: u8,
    ) -> Option<CaptureRegion> {
        if next == self.pending {
            self.streak = self.streak.saturating_add(1);
        } else {
            self.pending = next;
            self.streak = 1;
        }
        if self.active.is_none() || self.streak >= stable_frames {
            self.pending
        } else {
            self.active
        }
    }

    /// Check if a region reconfig is allowed (jitter filter).
    /// Returns true if the change is large enough or enough time has passed.
    pub fn should_reconfig(
        &self,
        committed: Option<CaptureRegion>,
        now: Instant,
        min_interval_ms: u64,
    ) -> bool {
        match (self.active, committed) {
            (Some(old), Some(new)) => {
                let small_jitter = old.x.abs_diff(new.x) <= 64
                    && old.y.abs_diff(new.y) <= 64
                    && old.w.abs_diff(new.w) <= 128
                    && old.h.abs_diff(new.h) <= 128;
                let too_soon = now.duration_since(self.last_reconfig_at)
                    < std::time::Duration::from_millis(min_interval_ms);
                !(small_jitter && too_soon)
            }
            _ => true,
        }
    }

    /// Apply a reconfig — update active region and timestamp.
    pub fn apply_reconfig(&mut self, region: Option<CaptureRegion>, now: Instant) {
        self.active = region;
        self.last_reconfig_at = now;
    }

    pub fn clear(&mut self, now: Instant) {
        self.active = None;
        self.pending = None;
        self.streak = 0;
        self.last_reconfig_at = now;
    }

    pub fn reset(&mut self) {
        self.active = None;
        self.pending = None;
        self.streak = 0;
        self.last_reconfig_at = Instant::now();
    }
}
