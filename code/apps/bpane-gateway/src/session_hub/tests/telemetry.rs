use std::sync::Arc;

use super::*;
use crate::session_hub::BrowserClientRole;

#[tokio::test]
async fn telemetry_tracks_join_latency_and_refresh_bursts() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, true).await.unwrap();

    let _owner = hub.subscribe().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let grid = TileMessage::GridConfig {
        tile_size: 64,
        cols: 3,
        rows: 2,
        screen_w: 192,
        screen_h: 128,
    }
    .to_frame();
    *hub.cached_grid_config.lock().await = Some(Arc::new(grid));

    let _viewer = hub.subscribe().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let snapshot = hub.telemetry_snapshot().await;
    assert_eq!(snapshot.browser_clients, 2);
    assert_eq!(snapshot.viewer_clients, 1);
    assert_eq!(snapshot.recorder_clients, 0);
    assert_eq!(snapshot.joins_accepted, 2);
    assert!(snapshot.max_join_latency_ms >= snapshot.last_join_latency_ms);
    assert_eq!(snapshot.full_refresh_requests, 1);
    assert_eq!(snapshot.full_refresh_tiles_requested, 6);
    assert_eq!(snapshot.last_full_refresh_tiles, 6);
    assert_eq!(snapshot.max_full_refresh_tiles, 6);
}

#[tokio::test]
async fn telemetry_reports_recorder_clients_separately() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, true).await.unwrap();

    let _owner = hub.subscribe().await.unwrap();
    let _recorder = hub
        .subscribe_with_role(BrowserClientRole::Recorder)
        .await
        .unwrap();

    let snapshot = hub.telemetry_snapshot().await;
    assert_eq!(snapshot.browser_clients, 2);
    assert_eq!(snapshot.viewer_clients, 0);
    assert_eq!(snapshot.recorder_clients, 1);
    assert_eq!(snapshot.viewer_slots_remaining, 10);
}
