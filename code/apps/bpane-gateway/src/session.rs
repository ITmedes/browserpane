use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::warn;

/// Session state for a single connected client.
pub struct Session {
    pub id: u64,
    /// Milliseconds since `session_start` at which the last heartbeat was received.
    /// Using AtomicU64 instead of Mutex<Instant> avoids async lock overhead on the hot path.
    last_heartbeat_ms: Arc<AtomicU64>,
    session_start: Instant,
    active: Arc<AtomicBool>,
    heartbeat_timeout: Duration,
}

impl Session {
    pub fn new(id: u64, heartbeat_timeout: Duration) -> Self {
        let session_start = Instant::now();
        Self {
            id,
            last_heartbeat_ms: Arc::new(AtomicU64::new(0)),
            session_start,
            active: Arc::new(AtomicBool::new(true)),
            heartbeat_timeout,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn deactivate(&self) {
        self.active.store(false, Ordering::Relaxed);
    }

    pub async fn update_heartbeat(&self) {
        let ms = self.session_start.elapsed().as_millis() as u64;
        self.last_heartbeat_ms.store(ms, Ordering::Relaxed);
    }

    pub async fn is_heartbeat_expired(&self) -> bool {
        let last_ms = self.last_heartbeat_ms.load(Ordering::Relaxed);
        let now_ms = self.session_start.elapsed().as_millis() as u64;
        let elapsed_ms = now_ms.saturating_sub(last_ms);
        elapsed_ms > self.heartbeat_timeout.as_millis() as u64
    }

    /// Run a heartbeat monitor that deactivates the session if no heartbeat
    /// is received within the timeout period.
    pub async fn run_heartbeat_monitor(self: Arc<Self>) {
        // Check at 1/3 of the timeout interval for responsiveness
        let check_interval = (self.heartbeat_timeout / 3).max(Duration::from_millis(10));
        loop {
            tokio::time::sleep(check_interval).await;
            if !self.is_active() {
                break;
            }
            if self.is_heartbeat_expired().await {
                warn!(
                    session_id = self.id,
                    "heartbeat timeout, deactivating session"
                );
                self.deactivate();
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn session_lifecycle() {
        let session = Session::new(1, Duration::from_secs(15));
        assert!(session.is_active());
        assert!(!session.is_heartbeat_expired().await);

        session.update_heartbeat().await;
        assert!(!session.is_heartbeat_expired().await);

        session.deactivate();
        assert!(!session.is_active());
    }

    #[tokio::test]
    async fn session_heartbeat_expires() {
        let session = Session::new(2, Duration::from_millis(50));
        assert!(!session.is_heartbeat_expired().await);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(session.is_heartbeat_expired().await);
    }

    #[tokio::test]
    async fn heartbeat_refresh_prevents_expiry() {
        let session = Session::new(3, Duration::from_millis(100));
        tokio::time::sleep(Duration::from_millis(60)).await;
        session.update_heartbeat().await;
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(!session.is_heartbeat_expired().await);
    }

    #[tokio::test]
    async fn heartbeat_monitor_deactivates_session() {
        let session = Arc::new(Session::new(4, Duration::from_millis(60)));
        let session_clone = session.clone();
        let handle = tokio::spawn(async move {
            session_clone.run_heartbeat_monitor().await;
        });
        // Wait for check_interval (20ms) + timeout (60ms) + some margin
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!session.is_active());
        handle.await.unwrap();
    }
}
