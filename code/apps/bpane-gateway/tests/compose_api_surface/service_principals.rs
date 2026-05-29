use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::{json, Value};

use super::support::{json_array, json_id, label_map, ComposeHarness};

const LOCAL_REALM_ISSUER: &str = "http://localhost:8091/realms/browserpane-dev";

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let invalid = harness
        .post_json_outcome(
            "/api/v1/service-principals",
            json!({
                "name": "",
                "client_id": "bpane-mcp-bridge",
                "issuer": LOCAL_REALM_ISSUER
            }),
        )
        .await?;
    if invalid.status != StatusCode::BAD_REQUEST {
        return Err(anyhow!(
            "invalid service principal create returned {} instead of 400: {}",
            invalid.status,
            invalid.body
        ));
    }

    let client_id = harness.unique_name("registry-client");
    let created = harness
        .post_json(
            "/api/v1/service-principals",
            json!({
                "name": harness.unique_name("compose-service-principal"),
                "description": "Compose e2e service principal",
                "client_id": client_id,
                "issuer": LOCAL_REALM_ISSUER,
                "labels": label_map("service-principals"),
                "scopes": ["session:delegate"],
                "allowed_project_ids": [],
                "state": "disabled"
            }),
        )
        .await?;
    let service_principal_id = json_id(&created, "id")?;
    if created["state"] != json!("disabled")
        || created["client_id"] != json!(client_id)
        || created["last_seen_at"] != Value::Null
        || created["last_delegated_at"] != Value::Null
    {
        return Err(anyhow!(
            "service principal create returned unexpected resource: {created}"
        ));
    }

    let duplicate = harness
        .post_json_outcome(
            "/api/v1/service-principals",
            json!({
                "name": "duplicate",
                "client_id": client_id,
                "issuer": LOCAL_REALM_ISSUER
            }),
        )
        .await?;
    if duplicate.status != StatusCode::CONFLICT {
        return Err(anyhow!(
            "duplicate service principal create returned {} instead of 409: {}",
            duplicate.status,
            duplicate.body
        ));
    }

    let listed = harness.get_json("/api/v1/service-principals").await?;
    let listed_service_principals = json_array(&listed, "service_principals")?;
    if !listed_service_principals
        .iter()
        .any(|candidate| candidate.get("id") == Some(&json!(service_principal_id)))
    {
        return Err(anyhow!(
            "service principal list did not include {service_principal_id}: {listed}"
        ));
    }

    let fetched = harness
        .get_json(&format!(
            "/api/v1/service-principals/{service_principal_id}"
        ))
        .await?;
    if fetched["id"] != json!(service_principal_id)
        || fetched["labels"]["scope"] != json!("service-principals")
    {
        return Err(anyhow!(
            "service principal get returned unexpected resource: {fetched}"
        ));
    }

    let session = harness.post_json("/api/v1/sessions", json!({})).await?;
    let session_id = json_id(&session, "id")?;
    let result = async {
        let blocked = harness
            .post_json_outcome(
                &format!("/api/v1/sessions/{session_id}/automation-owner"),
                json!({
                    "client_id": client_id,
                    "issuer": LOCAL_REALM_ISSUER,
                    "display_name": "Disabled registry client"
                }),
            )
            .await?;
        if blocked.status != StatusCode::CONFLICT
            || !blocked.body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("disabled")
        {
            return Err(anyhow!(
                "disabled service principal delegation returned unexpected result {}: {}",
                blocked.status,
                blocked.body
            ));
        }

        let updated = harness
            .put_json(
                &format!("/api/v1/service-principals/{service_principal_id}"),
                json!({
                    "name": created["name"],
                    "description": "Compose e2e service principal enabled",
                    "client_id": client_id,
                    "issuer": LOCAL_REALM_ISSUER,
                    "labels": label_map("service-principals"),
                    "scopes": ["session:delegate", "workflow:run"],
                    "allowed_project_ids": [],
                    "state": "active"
                }),
            )
            .await?;
        if updated["state"] != json!("active") || updated["scopes"][1] != json!("workflow:run") {
            return Err(anyhow!(
                "service principal update returned unexpected resource: {updated}"
            ));
        }

        let delegated = harness
            .post_json(
                &format!("/api/v1/sessions/{session_id}/automation-owner"),
                json!({
                    "client_id": client_id,
                    "issuer": LOCAL_REALM_ISSUER,
                    "display_name": "Enabled registry client"
                }),
            )
            .await?;
        if delegated["automation_delegate"]["client_id"] != json!(client_id) {
            return Err(anyhow!(
                "service principal delegation response did not expose delegate: {delegated}"
            ));
        }

        let reviewed = harness.get_json("/api/v1/identity/access-review").await?;
        if reviewed["resource_counts"]["service_principals"]
            .as_u64()
            .unwrap_or_default()
            < 1
        {
            return Err(anyhow!(
                "identity access review did not count registered service principals: {reviewed}"
            ));
        }
        let service_principals = json_array(&reviewed, "service_principals")?;
        let reviewed_service_principal = service_principals
            .iter()
            .find(|candidate| candidate.get("id") == Some(&json!(service_principal_id)))
            .ok_or_else(|| {
                anyhow!(
                    "identity access review did not include service principal {service_principal_id}: {reviewed}"
                )
            })?;
        if reviewed_service_principal["delegated_session_count"] != json!(1)
            || reviewed_service_principal["last_delegated_at"] == Value::Null
        {
            return Err(anyhow!(
                "identity access review did not correlate delegated service principal: {reviewed_service_principal}"
            ));
        }

        let delegates = json_array(&reviewed, "delegated_principals")?;
        let delegate = delegates
            .iter()
            .find(|candidate| candidate.get("client_id") == Some(&json!(client_id)))
            .ok_or_else(|| anyhow!("identity access review did not include delegate {client_id}: {reviewed}"))?;
        if delegate["registered"] != json!(true)
            || delegate["registered_service_principal_id"] != json!(service_principal_id)
        {
            return Err(anyhow!(
                "identity access review did not mark delegate as registered: {delegate}"
            ));
        }

        Ok(())
    }
    .await;

    let stop_result = harness.stop_session_eventually(&session_id).await;
    result?;
    stop_result?;
    Ok(())
}
