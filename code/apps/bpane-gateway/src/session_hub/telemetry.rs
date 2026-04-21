use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::SessionHub;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SessionTelemetrySnapshot {
    pub browser_clients: u32,
    pub viewer_clients: u32,
    pub max_viewers: u32,
    pub viewer_slots_remaining: u32,
    pub exclusive_browser_owner: bool,
    pub mcp_owner: bool,
    pub resolution: (u16, u16),
    pub joins_accepted: u64,
    pub joins_rejected_viewer_cap: u64,
    pub last_join_latency_ms: u64,
    pub average_join_latency_ms: f64,
    pub max_join_latency_ms: u64,
    pub full_refresh_requests: u64,
    pub full_refresh_tiles_requested: u64,
    pub last_full_refresh_tiles: u64,
    pub max_full_refresh_tiles: u64,
    pub egress_send_stream_lock_acquires_total: u64,
    pub egress_send_stream_lock_wait_us_total: u64,
    pub egress_send_stream_lock_wait_us_average: f64,
    pub egress_send_stream_lock_wait_us_max: u64,
    pub egress_lagged_receives_total: u64,
    pub egress_lagged_frames_total: u64,
}

pub(super) fn snapshot(hub: &SessionHub, resolution: (u16, u16)) -> SessionTelemetrySnapshot {
    let browser_clients = hub.client_count();
    let viewer_clients = hub.viewer_count();
    let joins_accepted = hub.joins_accepted.load(Ordering::Relaxed);
    let total_join_latency_ms = hub.total_join_latency_ms.load(Ordering::Relaxed);
    let egress_send_stream_lock_acquires_total =
        hub.egress_send_stream_lock_acquires_total.load(Ordering::Relaxed);
    let egress_send_stream_lock_wait_us_total =
        hub.egress_send_stream_lock_wait_us_total.load(Ordering::Relaxed);

    SessionTelemetrySnapshot {
        browser_clients,
        viewer_clients,
        max_viewers: hub.max_viewers,
        viewer_slots_remaining: hub.max_viewers.saturating_sub(viewer_clients),
        exclusive_browser_owner: hub.exclusive_browser_owner,
        mcp_owner: hub.mcp_is_owner(),
        resolution,
        joins_accepted,
        joins_rejected_viewer_cap: hub.joins_rejected_viewer_cap.load(Ordering::Relaxed),
        last_join_latency_ms: hub.last_join_latency_ms.load(Ordering::Relaxed),
        average_join_latency_ms: if joins_accepted == 0 {
            0.0
        } else {
            total_join_latency_ms as f64 / joins_accepted as f64
        },
        max_join_latency_ms: hub.max_join_latency_ms.load(Ordering::Relaxed),
        full_refresh_requests: hub.full_refresh_requests.load(Ordering::Relaxed),
        full_refresh_tiles_requested: hub.full_refresh_tiles_requested.load(Ordering::Relaxed),
        last_full_refresh_tiles: hub.last_full_refresh_tiles.load(Ordering::Relaxed),
        max_full_refresh_tiles: hub.max_full_refresh_tiles.load(Ordering::Relaxed),
        egress_send_stream_lock_acquires_total,
        egress_send_stream_lock_wait_us_total,
        egress_send_stream_lock_wait_us_average: if egress_send_stream_lock_acquires_total == 0 {
            0.0
        } else {
            egress_send_stream_lock_wait_us_total as f64
                / egress_send_stream_lock_acquires_total as f64
        },
        egress_send_stream_lock_wait_us_max: hub
            .egress_send_stream_lock_wait_us_max
            .load(Ordering::Relaxed),
        egress_lagged_receives_total: hub.egress_lagged_receives_total.load(Ordering::Relaxed),
        egress_lagged_frames_total: hub.egress_lagged_frames_total.load(Ordering::Relaxed),
    }
}

pub(super) fn record_join_latency(hub: &SessionHub, elapsed: Duration) {
    let join_ms = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
    hub.total_join_latency_ms
        .fetch_add(join_ms, Ordering::Relaxed);
    hub.last_join_latency_ms.store(join_ms, Ordering::Relaxed);
    update_max(&hub.max_join_latency_ms, join_ms);
}

pub(super) fn record_refresh_burst(hub: &SessionHub, tiles_requested: u64) {
    hub.full_refresh_requests.fetch_add(1, Ordering::Relaxed);
    hub.full_refresh_tiles_requested
        .fetch_add(tiles_requested, Ordering::Relaxed);
    hub.last_full_refresh_tiles
        .store(tiles_requested, Ordering::Relaxed);
    update_max(&hub.max_full_refresh_tiles, tiles_requested);
}

pub(super) fn record_egress_send_stream_lock_wait(hub: &SessionHub, elapsed: Duration) {
    let wait_us = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
    hub.egress_send_stream_lock_wait_us_total
        .fetch_add(wait_us, Ordering::Relaxed);
    hub.egress_send_stream_lock_acquires_total
        .fetch_add(1, Ordering::Relaxed);
    update_max(&hub.egress_send_stream_lock_wait_us_max, wait_us);
}

pub(super) fn record_egress_lagged(hub: &SessionHub, frames: u64) {
    hub.egress_lagged_receives_total.fetch_add(1, Ordering::Relaxed);
    hub.egress_lagged_frames_total
        .fetch_add(frames, Ordering::Relaxed);
}

fn update_max(target: &AtomicU64, value: u64) {
    let mut current = target.load(Ordering::Relaxed);
    while value > current {
        match target.compare_exchange(current, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}
