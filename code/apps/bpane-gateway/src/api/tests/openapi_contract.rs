use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use serde::Deserialize;
use tower::ServiceExt;

use super::test_router;

const CONTRACT_INVENTORY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../openapi/bpane-control-v1.operations.json"
));
const ROUTE_VALUE: &str = "00000000-0000-0000-0000-000000000001";

#[derive(Debug, Deserialize)]
struct ContractInventory {
    operations: Vec<ContractOperation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractOperation {
    operation_id: String,
    method: String,
    path: String,
}

#[tokio::test]
async fn openapi_contract_routes_are_registered() {
    let inventory: ContractInventory =
        serde_json::from_str(CONTRACT_INVENTORY).expect("OpenAPI operation inventory should parse");
    assert_eq!(inventory.operations.len(), 131);

    for operation in inventory.operations {
        let (router, _) = test_router();
        let path = materialize_path(&operation.path);
        let path_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::CONNECT)
                    .uri(&path)
                    .body(Body::empty())
                    .expect("route recognition request should build"),
            )
            .await
            .expect("route recognition request should complete");
        assert_eq!(
            path_response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "{} ({}) is documented but its path is not registered",
            operation.path,
            operation.operation_id
        );

        let method = Method::from_bytes(operation.method.as_bytes())
            .expect("inventory HTTP method should parse");
        let response = router
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(&path)
                    .body(Body::empty())
                    .expect("contract request should build"),
            )
            .await
            .expect("contract request should complete");

        assert_ne!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "{} {} ({}) has no matching method registration",
            operation.method,
            operation.path,
            operation.operation_id
        );
    }
}

fn materialize_path(template: &str) -> String {
    let mut path = template.to_string();
    while let Some(start) = path.find('{') {
        let end = path[start..]
            .find('}')
            .map(|offset| start + offset)
            .expect("OpenAPI route parameter should be closed");
        path.replace_range(start..=end, ROUTE_VALUE);
    }
    path
}

#[test]
fn path_materialization_replaces_every_parameter() {
    assert_eq!(
        materialize_path("/api/v1/sessions/{session_id}/connections/{connection_id}"),
        format!("/api/v1/sessions/{ROUTE_VALUE}/connections/{ROUTE_VALUE}")
    );
}
