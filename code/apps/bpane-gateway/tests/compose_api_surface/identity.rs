use anyhow::{anyhow, Result};
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde_json::{json, Value};

use super::support::{json_array, json_id, label_map, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    assert_unauthorized(
        harness
            .get_json_outcome_without_bearer("/api/v1/identity/me", HeaderMap::new())
            .await?,
        "identity me without bearer",
    )?;
    assert_unauthorized(
        harness
            .get_json_outcome_without_bearer("/api/v1/identity/access-review", HeaderMap::new())
            .await?,
        "identity access-review without bearer",
    )?;
    assert_unauthorized(
        harness
            .get_json_outcome_without_bearer("/api/v1/identity-mappings", HeaderMap::new())
            .await?,
        "identity mappings list without bearer",
    )?;
    assert_unauthorized(
        harness
            .post_json_outcome_without_bearer(
                "/api/v1/identity-mappings",
                json!({
                    "name": "unauthorized mapping",
                    "kind": "user",
                    "issuer": "http://localhost:8091/realms/browserpane-dev",
                    "external_id": "unauthorized",
                    "project_id": "00000000-0000-0000-0000-000000000001"
                }),
                HeaderMap::new(),
            )
            .await?,
        "identity mapping create without bearer",
    )?;

    let identity = harness.get_json("/api/v1/identity/me").await?;
    let subject = identity
        .get("subject")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("identity response did not include a subject: {identity}"))?;
    let issuer = identity
        .get("issuer")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("identity response did not include an issuer: {identity}"))?;
    if subject.is_empty() || issuer.is_empty() {
        return Err(anyhow!(
            "identity response returned an empty subject or issuer: {identity}"
        ));
    }
    if identity["principal_type"] != json!("service_principal") {
        return Err(anyhow!(
            "compose client credentials principal was not classified as a service principal: {identity}"
        ));
    }

    let project = harness
        .post_json(
            "/api/v1/projects",
            json!({
                "name": harness.unique_name("identity-review-project"),
                "description": "Compose e2e identity access review project",
                "labels": label_map("identity-access-review"),
                "quotas": {
                    "max_active_sessions": 2,
                    "max_active_workflow_runs": 2,
                    "max_retained_storage_bytes": 1048576
                }
            }),
        )
        .await?;
    let project_id = json_id(&project, "id")?;
    let session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "project_id": project_id,
                "labels": {
                    "scope": "identity-access-review"
                }
            }),
        )
        .await?;
    let session_id = json_id(&session, "id")?;

    let result = async {
        let invalid_mapping = harness
            .post_json_outcome(
                "/api/v1/identity-mappings",
                json!({
                    "name": "",
                    "kind": "user",
                    "issuer": issuer,
                    "external_id": subject,
                    "project_id": project_id
                }),
            )
            .await?;
        if invalid_mapping.status != StatusCode::BAD_REQUEST {
            return Err(anyhow!(
                "invalid identity mapping create returned {} instead of 400: {}",
                invalid_mapping.status,
                invalid_mapping.body
            ));
        }

        let mapping_body = json!({
            "name": harness.unique_name("identity-project-mapping"),
            "description": "Compose e2e identity-to-project mapping",
            "kind": "user",
            "issuer": issuer,
            "external_id": subject,
            "project_id": project_id,
            "labels": label_map("identity-mappings"),
            "scopes": ["session:create"],
            "state": "active"
        });
        let mapping = harness
            .post_json("/api/v1/identity-mappings", mapping_body.clone())
            .await?;
        let mapping_id = json_id(&mapping, "id")?;
        if mapping["kind"] != json!("user")
            || mapping["issuer"] != json!(issuer)
            || mapping["external_id"] != json!(subject)
            || mapping["project_id"] != json!(project_id)
            || mapping["state"] != json!("active")
        {
            return Err(anyhow!(
                "identity mapping create returned unexpected resource: {mapping}"
            ));
        }

        let duplicate = harness
            .post_json_outcome("/api/v1/identity-mappings", mapping_body)
            .await?;
        if duplicate.status != StatusCode::CONFLICT {
            return Err(anyhow!(
                "duplicate identity mapping create returned {} instead of 409: {}",
                duplicate.status,
                duplicate.body
            ));
        }

        let listed = harness.get_json("/api/v1/identity-mappings").await?;
        let listed_mappings = json_array(&listed, "identity_mappings")?;
        if !listed_mappings
            .iter()
            .any(|candidate| candidate.get("id") == Some(&json!(mapping_id)))
        {
            return Err(anyhow!(
                "identity mapping list did not include {mapping_id}: {listed}"
            ));
        }

        let fetched = harness
            .get_json(&format!("/api/v1/identity-mappings/{mapping_id}"))
            .await?;
        if fetched["id"] != json!(mapping_id)
            || fetched["labels"]["scope"] != json!("identity-mappings")
        {
            return Err(anyhow!(
                "identity mapping get returned unexpected resource: {fetched}"
            ));
        }

        let disabled = harness
            .put_json(
                &format!("/api/v1/identity-mappings/{mapping_id}"),
                json!({
                    "name": fetched["name"],
                    "description": "Compose e2e identity mapping disabled",
                    "kind": "user",
                    "issuer": issuer,
                    "external_id": subject,
                    "project_id": project_id,
                    "labels": label_map("identity-mappings"),
                    "scopes": ["session:create"],
                    "state": "disabled"
                }),
            )
            .await?;
        if disabled["state"] != json!("disabled") {
            return Err(anyhow!(
                "identity mapping disable update did not persist disabled state: {disabled}"
            ));
        }

        let enabled = harness
            .put_json(
                &format!("/api/v1/identity-mappings/{mapping_id}"),
                json!({
                    "name": disabled["name"],
                    "description": "Compose e2e identity mapping enabled",
                    "kind": "user",
                    "issuer": issuer,
                    "external_id": subject,
                    "project_id": project_id,
                    "labels": label_map("identity-mappings"),
                    "scopes": ["session:create", "workflow:run"],
                    "state": "active"
                }),
            )
            .await?;
        if enabled["state"] != json!("active") || enabled["scopes"][1] != json!("workflow:run") {
            return Err(anyhow!(
                "identity mapping update did not re-enable mapping with scopes: {enabled}"
            ));
        }

        let delegated = harness
            .post_json(
                &format!("/api/v1/sessions/{session_id}/automation-owner"),
                json!({
                    "client_id": "bpane-mcp-bridge",
                    "issuer": "http://localhost:8091/realms/browserpane-dev",
                    "display_name": "BrowserPane MCP bridge"
                }),
            )
            .await?;
        if delegated["automation_delegate"]["client_id"] != json!("bpane-mcp-bridge") {
            return Err(anyhow!(
                "automation delegation response did not expose the delegated principal: {delegated}"
            ));
        }

        let review = harness.get_json("/api/v1/identity/access-review").await?;
        if review["principal"]["subject"] != json!(subject)
            || review["principal"]["issuer"] != json!(issuer)
            || review["principal"]["principal_type"] != json!("service_principal")
        {
            return Err(anyhow!(
                "identity access review principal did not match /identity/me: {review}"
            ));
        }
        assert_count_at_least(&review, "projects", 1)?;
        assert_count_at_least(&review, "sessions", 1)?;
        assert_count_at_least(&review, "active_sessions", 1)?;
        assert_count_at_least(&review, "delegated_principals", 1)?;
        assert_count_at_least(&review, "identity_mappings", 1)?;

        let projects = json_array(&review, "projects")?;
        let reviewed_project = projects
            .iter()
            .find(|candidate| candidate.get("id") == Some(&json!(project_id)))
            .ok_or_else(|| {
                anyhow!(
                    "identity access review did not include created project {project_id}: {review}"
                )
            })?;
        if reviewed_project["usage"]["active_sessions"] != json!(1) {
            return Err(anyhow!(
                "identity access review did not count active project usage: {reviewed_project}"
            ));
        }

        let identity_mappings = json_array(&review, "identity_mappings")?;
        let reviewed_mapping = identity_mappings
            .iter()
            .find(|candidate| candidate.get("id") == Some(&json!(mapping_id)))
            .ok_or_else(|| {
                anyhow!(
                    "identity access review did not include identity mapping {mapping_id}: {review}"
                )
            })?;
        if reviewed_mapping["effective_for_principal"] != json!(true)
            || reviewed_mapping["scopes"][1] != json!("workflow:run")
        {
            return Err(anyhow!(
                "identity access review did not mark mapping effective with updated scopes: {reviewed_mapping}"
            ));
        }

        let unmapped_principal_signals = json_array(&review, "unmapped_principal_signals")?;
        if unmapped_principal_signals.iter().any(|candidate| {
            candidate.get("kind") == Some(&json!("user"))
                && candidate.get("issuer") == Some(&json!(issuer))
                && candidate.get("external_id") == Some(&json!(subject))
        }) {
            return Err(anyhow!(
                "identity access review still reported current principal as unmapped: {review}"
            ));
        }

        let delegated_principals = json_array(&review, "delegated_principals")?;
        let delegated_principal = delegated_principals
            .iter()
            .find(|candidate| {
                candidate.get("client_id") == Some(&json!("bpane-mcp-bridge"))
                    && candidate
                        .get("session_ids")
                        .and_then(Value::as_array)
                        .is_some_and(|session_ids| session_ids.contains(&json!(session_id)))
            })
            .ok_or_else(|| {
                anyhow!(
                    "identity access review did not include delegated session {session_id}: {review}"
                )
            })?;
        if delegated_principal["active_session_count"]
            .as_u64()
            .unwrap_or_default()
            < 1
        {
            return Err(anyhow!(
                "identity access review did not count active delegated sessions: {delegated_principal}"
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

fn assert_unauthorized(outcome: super::support::JsonOutcome, context: &str) -> Result<()> {
    if outcome.status != StatusCode::UNAUTHORIZED {
        return Err(anyhow!(
            "{context} returned {} instead of 401: {}",
            outcome.status,
            outcome.body
        ));
    }
    Ok(())
}

fn assert_count_at_least(review: &Value, field: &str, minimum: u64) -> Result<()> {
    let count = review["resource_counts"][field]
        .as_u64()
        .ok_or_else(|| anyhow!("identity access review count {field} was missing: {review}"))?;
    if count < minimum {
        return Err(anyhow!(
            "identity access review count {field} was {count}, expected at least {minimum}: {review}"
        ));
    }
    Ok(())
}
