use super::*;
use crate::capture::ffmpeg::CaptureRegion;
use std::time::{Duration, Instant};

fn region(x: u32, y: u32, w: u32, h: u32) -> CaptureRegion {
    CaptureRegion { x, y, w, h }
}

// ── EditableHintState ───────────────────────────────────────────────

#[test]
fn editable_hint_update_stores_fresh_hint() {
    let mut state = EditableHintState::new();
    let now = Instant::now();
    let r = region(10, 20, 100, 30);
    state.update(Some(r), now, 500);
    assert_eq!(state.last_hint, Some(r));
    assert!(state.last_hint_at.is_some());
}

#[test]
fn editable_hint_expires_after_hold_ms() {
    let mut state = EditableHintState::new();
    let t0 = Instant::now();
    state.update(Some(region(0, 0, 50, 20)), t0, 100);
    assert!(state.last_hint.is_some());

    // Simulate time passing beyond hold_ms
    let t1 = t0 + Duration::from_millis(200);
    state.update(None, t1, 100);
    assert!(state.last_hint.is_none());
}

#[test]
fn editable_hint_holds_within_window() {
    let mut state = EditableHintState::new();
    let t0 = Instant::now();
    state.update(Some(region(0, 0, 50, 20)), t0, 500);

    // Still within hold window
    let t1 = t0 + Duration::from_millis(200);
    state.update(None, t1, 500);
    assert!(state.last_hint.is_some(), "hint should be held within window");
}

#[test]
fn key_input_qoi_boost_active_within_window() {
    let mut state = EditableHintState::new();
    let now = Instant::now();
    state.on_key_input(now);
    assert!(state.key_input_qoi_boost(now, 800));
    assert!(state.key_input_qoi_boost(now + Duration::from_millis(500), 800));
    assert!(!state.key_input_qoi_boost(now + Duration::from_millis(900), 800));
}

#[test]
fn qoi_region_prefers_cdp_hint() {
    let state = EditableHintState::new();
    let now = Instant::now();
    let r = region(10, 20, 50, 30);
    assert_eq!(state.qoi_region(Some(r), now, 500, 800), Some(r));
}

#[test]
fn qoi_region_falls_back_to_held_during_boost() {
    let mut state = EditableHintState::new();
    let t0 = Instant::now();
    let r = region(10, 20, 50, 30);
    state.update(Some(r), t0, 500);
    state.on_key_input(t0);

    // No CDP hint but within boost + hold windows
    let t1 = t0 + Duration::from_millis(200);
    assert_eq!(state.qoi_region(None, t1, 500, 800), Some(r));
}

#[test]
fn qoi_region_none_without_boost() {
    let mut state = EditableHintState::new();
    let t0 = Instant::now();
    state.update(Some(region(10, 20, 50, 30)), t0, 500);
    // No key input → no boost → None fallback
    let t1 = t0 + Duration::from_millis(200);
    assert_eq!(state.qoi_region(None, t1, 500, 800), None);
}

// ── ClickArmedState ─────────────────────────────────────────────────

#[test]
fn click_armed_latches_on_matching_click() {
    let mut state = ClickArmedState::new();
    let now = Instant::now();
    let r = region(100, 100, 200, 200);
    let armed = state.update(Some(r), Some((150, 150, now)), now, 5000, 20);
    assert!(armed);
}

#[test]
fn click_armed_stays_latched_while_hints_present() {
    let mut state = ClickArmedState::new();
    let now = Instant::now();
    let r = region(100, 100, 200, 200);
    state.update(Some(r), Some((150, 150, now)), now, 5000, 20);

    // Next frame, no click but hint still present
    let armed = state.update(Some(r), None, now, 5000, 20);
    assert!(armed, "should stay latched");
}

#[test]
fn click_armed_resets_after_absent_streak() {
    let mut state = ClickArmedState::new();
    let now = Instant::now();
    let r = region(100, 100, 200, 200);
    state.update(Some(r), Some((150, 150, now)), now, 5000, 3);

    // 3 frames without hint → reset
    state.update(None, None, now, 5000, 3);
    state.update(None, None, now, 5000, 3);
    let armed = state.update(None, None, now, 5000, 3);
    assert!(!armed, "should reset after absent streak");
}

#[test]
fn click_armed_not_armed_without_click() {
    let mut state = ClickArmedState::new();
    let now = Instant::now();
    let r = region(100, 100, 200, 200);
    let armed = state.update(Some(r), None, now, 5000, 20);
    assert!(!armed);
}

// ── RegionCommitter ─────────────────────────────────────────────────

#[test]
fn region_committer_accepts_first_region_immediately() {
    let mut rc = RegionCommitter::new();
    let r = region(0, 0, 640, 360);
    let committed = rc.commit(Some(r), 2);
    assert_eq!(committed, Some(r));
}

#[test]
fn region_committer_requires_stable_frames() {
    let mut rc = RegionCommitter::new();
    let r1 = region(0, 0, 640, 360);
    rc.active = Some(r1);
    let r2 = region(10, 10, 640, 360);

    // First frame with new region — streak=1, not yet stable
    let c1 = rc.commit(Some(r2), 3);
    assert_eq!(c1, Some(r1), "should keep old region until stable");

    // Second frame — streak=2
    let c2 = rc.commit(Some(r2), 3);
    assert_eq!(c2, Some(r1));

    // Third frame — streak=3, now stable
    let c3 = rc.commit(Some(r2), 3);
    assert_eq!(c3, Some(r2));
}

#[test]
fn region_committer_jitter_filter_blocks_small_fast_changes() {
    let mut rc = RegionCommitter::new();
    let r = region(100, 100, 640, 360);
    rc.active = Some(r);
    rc.last_reconfig_at = Instant::now();

    // Small jitter + too soon → blocked
    let tweaked = region(110, 105, 650, 370);
    assert!(!rc.should_reconfig(Some(tweaked), Instant::now(), 500));
}

#[test]
fn region_committer_allows_large_changes() {
    let mut rc = RegionCommitter::new();
    let r = region(100, 100, 640, 360);
    rc.active = Some(r);
    rc.last_reconfig_at = Instant::now();

    // Large change → allowed even if recent
    let big_move = region(500, 400, 640, 360);
    assert!(rc.should_reconfig(Some(big_move), Instant::now(), 500));
}

#[test]
fn region_committer_reset_clears_state() {
    let mut rc = RegionCommitter::new();
    rc.active = Some(region(0, 0, 100, 100));
    rc.reset();
    assert!(rc.active.is_none());
}

#[test]
fn region_committer_apply_reconfig_updates_state() {
    let mut rc = RegionCommitter::new();
    let r = region(100, 200, 640, 360);
    let now = Instant::now();
    rc.apply_reconfig(Some(r), now);
    assert_eq!(rc.active, Some(r));
    assert_eq!(rc.last_reconfig_at, now);
}

#[test]
fn region_committer_apply_reconfig_to_none() {
    let mut rc = RegionCommitter::new();
    rc.active = Some(region(10, 10, 100, 100));
    let now = Instant::now();
    rc.apply_reconfig(None, now);
    assert!(rc.active.is_none());
    assert_eq!(rc.last_reconfig_at, now);
}
