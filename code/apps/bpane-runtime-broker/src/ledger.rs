use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bpane_runtime_contract::RuntimeOperationResponse;
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

/// Bounded idempotency ledger settings.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct LedgerConfig {
    /// Maximum pending and completed operations retained at once.
    pub capacity: NonZeroUsize,
    /// Retention window for completed results.
    pub completed_ttl: Duration,
}

/// Decision returned when an operation enters the ledger.
pub enum LedgerDecision {
    /// This caller owns first execution of the operation.
    Execute,
    /// A completed exact retry can reuse this result.
    Cached(RuntimeOperationResponse),
    /// An identical request is already executing.
    Wait(Arc<Notify>),
    /// The key was reused for different request content.
    IdempotencyConflict,
    /// The request id was reused with another idempotency key.
    ReplayConflict,
    /// Capacity remains exhausted after expired results were pruned.
    CapacityExceeded,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct ScopedKey {
    subject: String,
    idempotency_key: String,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct ScopedRequestId {
    subject: String,
    request_id: Uuid,
}

enum EntryState {
    Pending(Arc<Notify>),
    Completed {
        response: RuntimeOperationResponse,
        completed_at: Instant,
    },
}

struct LedgerEntry {
    request_id: Uuid,
    fingerprint: [u8; 32],
    state: EntryState,
}

#[derive(Default)]
struct LedgerInner {
    entries: HashMap<ScopedKey, LedgerEntry>,
    request_ids: HashMap<ScopedRequestId, ScopedKey>,
}

/// Principal-scoped, bounded operation idempotency and replay ledger.
pub struct OperationLedger {
    config: LedgerConfig,
    inner: Mutex<LedgerInner>,
}

impl OperationLedger {
    /// Creates an empty ledger with fixed capacity and retention.
    pub fn new(config: LedgerConfig) -> Self {
        Self {
            config,
            inner: Mutex::new(LedgerInner::default()),
        }
    }

    /// Begins or resolves one principal-scoped operation.
    pub async fn begin(
        &self,
        subject: &str,
        idempotency_key: &str,
        request_id: Uuid,
        fingerprint: [u8; 32],
    ) -> LedgerDecision {
        let mut inner = self.inner.lock().await;
        prune_expired(&mut inner, self.config.completed_ttl);
        let key = ScopedKey {
            subject: subject.to_string(),
            idempotency_key: idempotency_key.to_string(),
        };
        let scoped_request_id = ScopedRequestId {
            subject: subject.to_string(),
            request_id,
        };
        if let Some(existing_key) = inner.request_ids.get(&scoped_request_id) {
            if existing_key != &key {
                return LedgerDecision::ReplayConflict;
            }
        }
        if let Some(entry) = inner.entries.get(&key) {
            if entry.request_id != request_id || entry.fingerprint != fingerprint {
                return LedgerDecision::IdempotencyConflict;
            }
            return match &entry.state {
                EntryState::Pending(notify) => LedgerDecision::Wait(Arc::clone(notify)),
                EntryState::Completed { response, .. } => LedgerDecision::Cached(response.clone()),
            };
        }
        if inner.entries.len() >= self.config.capacity.get() {
            return LedgerDecision::CapacityExceeded;
        }
        let notify = Arc::new(Notify::new());
        inner.request_ids.insert(scoped_request_id, key.clone());
        inner.entries.insert(
            key,
            LedgerEntry {
                request_id,
                fingerprint,
                state: EntryState::Pending(notify),
            },
        );
        LedgerDecision::Execute
    }

    /// Completes an operation and wakes exact retries.
    pub async fn complete(
        &self,
        subject: &str,
        idempotency_key: &str,
        response: RuntimeOperationResponse,
    ) {
        let mut inner = self.inner.lock().await;
        let key = ScopedKey {
            subject: subject.to_string(),
            idempotency_key: idempotency_key.to_string(),
        };
        let Some(entry) = inner.entries.get_mut(&key) else {
            return;
        };
        let notify = match &entry.state {
            EntryState::Pending(notify) => Arc::clone(notify),
            EntryState::Completed { .. } => return,
        };
        entry.state = EntryState::Completed {
            response,
            completed_at: Instant::now(),
        };
        notify.notify_waiters();
    }

    /// Aborts a pending operation so a later retry can execute it again.
    pub async fn abort(&self, subject: &str, idempotency_key: &str) {
        let mut inner = self.inner.lock().await;
        let key = ScopedKey {
            subject: subject.to_string(),
            idempotency_key: idempotency_key.to_string(),
        };
        let Some(entry) = inner.entries.remove(&key) else {
            return;
        };
        inner.request_ids.remove(&ScopedRequestId {
            subject: subject.to_string(),
            request_id: entry.request_id,
        });
        if let EntryState::Pending(notify) = entry.state {
            notify.notify_waiters();
        }
    }
}

fn prune_expired(inner: &mut LedgerInner, completed_ttl: Duration) {
    let now = Instant::now();
    let expired: Vec<_> = inner
        .entries
        .iter()
        .filter_map(|(key, entry)| match entry.state {
            EntryState::Completed { completed_at, .. }
                if now.saturating_duration_since(completed_at) >= completed_ttl =>
            {
                Some((key.clone(), entry.request_id))
            }
            _ => None,
        })
        .collect();
    for (key, request_id) in expired {
        inner.entries.remove(&key);
        inner.request_ids.remove(&ScopedRequestId {
            subject: key.subject.clone(),
            request_id,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bpane_runtime_contract::{BrokerApiVersion, RuntimeOperationResult};

    fn config(capacity: usize) -> LedgerConfig {
        LedgerConfig {
            capacity: NonZeroUsize::new(capacity).unwrap(),
            completed_ttl: Duration::from_millis(5),
        }
    }

    fn response(request_id: Uuid) -> RuntimeOperationResponse {
        RuntimeOperationResponse {
            api_version: BrokerApiVersion::V1,
            request_id,
            result: RuntimeOperationResult::Accepted,
        }
    }

    #[tokio::test]
    async fn exact_retry_waits_then_reuses_completed_result() {
        let ledger = OperationLedger::new(config(2));
        let request_id = Uuid::now_v7();
        let fingerprint = [1; 32];
        assert!(matches!(
            ledger
                .begin("gateway", "key", request_id, fingerprint)
                .await,
            LedgerDecision::Execute
        ));
        assert!(matches!(
            ledger
                .begin("gateway", "key", request_id, fingerprint)
                .await,
            LedgerDecision::Wait(_)
        ));
        ledger
            .complete("gateway", "key", response(request_id))
            .await;
        assert!(matches!(
            ledger
                .begin("gateway", "key", request_id, fingerprint)
                .await,
            LedgerDecision::Cached(_)
        ));
    }

    #[tokio::test]
    async fn rejects_conflicting_key_and_request_id_reuse() {
        let ledger = OperationLedger::new(config(3));
        let request_id = Uuid::now_v7();
        assert!(matches!(
            ledger.begin("gateway", "key", request_id, [1; 32]).await,
            LedgerDecision::Execute
        ));
        assert!(matches!(
            ledger.begin("gateway", "key", request_id, [2; 32]).await,
            LedgerDecision::IdempotencyConflict
        ));
        assert!(matches!(
            ledger
                .begin("gateway", "other-key", request_id, [1; 32])
                .await,
            LedgerDecision::ReplayConflict
        ));
    }

    #[tokio::test]
    async fn scopes_keys_by_principal_and_enforces_capacity() {
        let ledger = OperationLedger::new(config(2));
        assert!(matches!(
            ledger
                .begin("gateway-a", "key", Uuid::now_v7(), [1; 32])
                .await,
            LedgerDecision::Execute
        ));
        assert!(matches!(
            ledger
                .begin("gateway-b", "key", Uuid::now_v7(), [1; 32])
                .await,
            LedgerDecision::Execute
        ));
        assert!(matches!(
            ledger
                .begin("gateway-c", "key", Uuid::now_v7(), [1; 32])
                .await,
            LedgerDecision::CapacityExceeded
        ));
    }

    #[tokio::test]
    async fn abort_and_expiry_release_capacity() {
        let ledger = OperationLedger::new(config(1));
        let first = Uuid::now_v7();
        ledger.begin("gateway", "one", first, [1; 32]).await;
        ledger.abort("gateway", "one").await;
        let second = Uuid::now_v7();
        ledger.begin("gateway", "two", second, [2; 32]).await;
        ledger.complete("gateway", "two", response(second)).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(matches!(
            ledger
                .begin("gateway", "three", Uuid::now_v7(), [3; 32])
                .await,
            LedgerDecision::Execute
        ));
    }
}
