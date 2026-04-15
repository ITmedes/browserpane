use std::sync::Arc;

use super::*;

#[test]
fn datagram_stats_initial_zero() {
    let stats = DatagramStats::new();
    let (successes, failures) = stats.take_counts();
    assert_eq!(successes, 0);
    assert_eq!(failures, 0);
}

#[test]
fn datagram_stats_counts_success_and_failure() {
    let stats = DatagramStats::new();
    stats.record_success();
    stats.record_success();
    stats.record_success();
    stats.record_failure();

    let (successes, failures) = stats.take_counts();
    assert_eq!(successes, 3);
    assert_eq!(failures, 1);
}

#[test]
fn datagram_stats_take_resets_counters() {
    let stats = DatagramStats::new();
    stats.record_success();
    stats.record_failure();

    let (successes, failures) = stats.take_counts();
    assert_eq!(successes, 1);
    assert_eq!(failures, 1);

    let (successes, failures) = stats.take_counts();
    assert_eq!(successes, 0);
    assert_eq!(failures, 0);
}

#[test]
fn datagram_stats_concurrent_access() {
    let stats = Arc::new(DatagramStats::new());
    let mut handles = Vec::new();
    for _ in 0..10 {
        let stats = Arc::clone(&stats);
        handles.push(std::thread::spawn(move || {
            for _ in 0..100 {
                stats.record_success();
                stats.record_failure();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let (successes, failures) = stats.take_counts();
    assert_eq!(successes, 1000);
    assert_eq!(failures, 1000);
}

#[test]
fn bitrate_adapts_down_on_high_failure() {
    assert_eq!(compute_adapted_bitrate(2_000_000, 80, 20), 1_600_000);
}

#[test]
fn bitrate_adapts_down_on_moderate_failure() {
    assert_eq!(compute_adapted_bitrate(2_000_000, 95, 5), 1_900_000);
}

#[test]
fn bitrate_adapts_up_on_zero_failure() {
    assert_eq!(compute_adapted_bitrate(2_000_000, 100, 0), 2_100_000);
}

#[test]
fn bitrate_stays_same_on_low_failure() {
    assert_eq!(compute_adapted_bitrate(2_000_000, 99, 1), 2_000_000);
}

#[test]
fn bitrate_clamps_to_minimum() {
    let mut bitrate_bps = 300_000u32;
    for _ in 0..10 {
        bitrate_bps = compute_adapted_bitrate(bitrate_bps, 5, 50);
    }
    assert!(bitrate_bps >= 200_000);
}

#[test]
fn bitrate_clamps_to_maximum() {
    let mut bitrate_bps = 7_500_000u32;
    for _ in 0..50 {
        bitrate_bps = compute_adapted_bitrate(bitrate_bps, 100, 0);
    }
    assert!(bitrate_bps <= 8_000_000);
}

#[test]
fn bitrate_no_change_on_zero_traffic() {
    assert_eq!(compute_adapted_bitrate(2_000_000, 0, 0), 2_000_000);
}
