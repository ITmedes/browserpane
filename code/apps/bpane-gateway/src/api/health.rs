use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::lifecycle::{GatewayLifecycle, GatewayLifecycleState};
use crate::readiness::{GatewayReadiness, GatewayReadinessSnapshot, ReadinessStatus};

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    lifecycle: GatewayLifecycleState,
}

#[derive(Clone)]
struct HealthState {
    lifecycle: Arc<GatewayLifecycle>,
    readiness: Arc<GatewayReadiness>,
}

pub(super) fn health_routes(
    lifecycle: Arc<GatewayLifecycle>,
    readiness: Arc<GatewayReadiness>,
) -> Router {
    Router::new()
        .route("/healthz", get(get_health))
        .route("/readyz", get(get_readiness))
        .with_state(HealthState {
            lifecycle,
            readiness,
        })
}

async fn get_health(State(state): State<HealthState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "live",
        lifecycle: state.lifecycle.state(),
    })
}

async fn get_readiness(
    State(state): State<HealthState>,
) -> (StatusCode, Json<GatewayReadinessSnapshot>) {
    let snapshot = state.readiness.snapshot().await;
    let status = if snapshot.status == ReadinessStatus::Ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(snapshot))
}
