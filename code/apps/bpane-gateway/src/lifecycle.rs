use std::sync::atomic::{AtomicU8, Ordering};

use serde::Serialize;
use tokio::sync::watch;

const STARTING: u8 = 0;
const RUNNING: u8 = 1;
const DRAINING: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GatewayLifecycleState {
    Starting,
    Running,
    Draining,
}

impl GatewayLifecycleState {
    fn as_u8(self) -> u8 {
        match self {
            Self::Starting => STARTING,
            Self::Running => RUNNING,
            Self::Draining => DRAINING,
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            RUNNING => Self::Running,
            DRAINING => Self::Draining,
            _ => Self::Starting,
        }
    }
}

pub(crate) struct GatewayLifecycle {
    state: AtomicU8,
    state_tx: watch::Sender<GatewayLifecycleState>,
}

impl GatewayLifecycle {
    pub(crate) fn new() -> Self {
        let (state_tx, _) = watch::channel(GatewayLifecycleState::Starting);
        Self {
            state: AtomicU8::new(STARTING),
            state_tx,
        }
    }

    pub(crate) fn state(&self) -> GatewayLifecycleState {
        GatewayLifecycleState::from_u8(self.state.load(Ordering::Acquire))
    }

    pub(crate) fn mark_running(&self) -> bool {
        self.transition(
            GatewayLifecycleState::Starting,
            GatewayLifecycleState::Running,
        )
    }

    pub(crate) fn begin_draining(&self) -> bool {
        loop {
            let current = self.state();
            if current == GatewayLifecycleState::Draining {
                return false;
            }
            if self.transition(current, GatewayLifecycleState::Draining) {
                return true;
            }
        }
    }

    pub(crate) fn accepts_new_work(&self) -> bool {
        self.state() == GatewayLifecycleState::Running
    }

    pub(crate) async fn wait_for_draining(&self) {
        if self.state() == GatewayLifecycleState::Draining {
            return;
        }
        let mut state_rx = self.state_tx.subscribe();
        while state_rx.changed().await.is_ok() {
            if *state_rx.borrow_and_update() == GatewayLifecycleState::Draining {
                return;
            }
        }
    }

    fn transition(&self, current: GatewayLifecycleState, next: GatewayLifecycleState) -> bool {
        if self
            .state
            .compare_exchange(
                current.as_u8(),
                next.as_u8(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return false;
        }
        self.state_tx.send_replace(next);
        true
    }
}

impl Default for GatewayLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn lifecycle_transitions_are_monotonic() {
        let lifecycle = GatewayLifecycle::new();

        assert_eq!(lifecycle.state(), GatewayLifecycleState::Starting);
        assert!(lifecycle.mark_running());
        assert!(lifecycle.accepts_new_work());
        assert!(lifecycle.begin_draining());
        assert!(!lifecycle.begin_draining());
        assert!(!lifecycle.mark_running());
        assert!(!lifecycle.accepts_new_work());
        assert_eq!(lifecycle.state(), GatewayLifecycleState::Draining);
    }

    #[tokio::test]
    async fn drain_waiters_observe_the_transition() {
        let lifecycle = Arc::new(GatewayLifecycle::new());
        let waiter_lifecycle = lifecycle.clone();
        let waiter = tokio::spawn(async move {
            waiter_lifecycle.wait_for_draining().await;
        });

        assert!(lifecycle.mark_running());
        assert!(lifecycle.begin_draining());
        waiter.await.unwrap();
    }
}
