use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::json;

use super::support::{json_array, json_id, label_map, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let invalid = harness
        .post_json_outcome(
            "/api/v1/projects",
            json!({
                "name": "invalid-project",
                "quotas": { "max_active_sessions": 0 }
            }),
        )
        .await?;
    if invalid.status != StatusCode::BAD_REQUEST {
        return Err(anyhow!(
            "invalid project create returned {} instead of 400: {}",
            invalid.status,
            invalid.body
        ));
    }
    let invalid_policy = harness
        .post_json_outcome(
            "/api/v1/projects",
            json!({
                "name": "invalid-project-policy",
                "policy": {
                    "allowed_session_template_ids": [" "]
                }
            }),
        )
        .await?;
    if invalid_policy.status != StatusCode::BAD_REQUEST {
        return Err(anyhow!(
            "invalid project policy create returned {} instead of 400: {}",
            invalid_policy.status,
            invalid_policy.body
        ));
    }

    let project_name = harness.unique_name("compose-project");
    let project = harness
        .post_json(
            "/api/v1/projects",
            json!({
                "name": project_name,
                "description": "Compose e2e project",
                "labels": label_map("projects"),
                "quotas": {
                    "max_active_sessions": 1,
                    "max_active_workflow_runs": 2,
                    "max_retained_storage_bytes": 1048576
                }
            }),
        )
        .await?;
    let project_id = json_id(&project, "id")?;
    if project["state"] != json!("active")
        || project["usage"]["active_sessions"] != json!(0)
        || project["usage"]["max_active_sessions"] != json!(1)
        || project["policy"]["allowed_session_template_ids"] != json!([])
        || project["policy"]["allowed_egress_profile_ids"] != json!([])
    {
        return Err(anyhow!(
            "project create returned unexpected resource: {project}"
        ));
    }

    let allowed_profile = harness
        .post_json(
            "/api/v1/egress-profiles",
            json!({
                "name": harness.unique_name("compose-project-egress-allowed"),
                "description": "Compose e2e project policy allowed egress",
                "labels": label_map("projects"),
                "proxy": { "url": "https://proxy.example:8443" }
            }),
        )
        .await?;
    let allowed_profile_id = json_id(&allowed_profile, "id")?;
    let disallowed_profile = harness
        .post_json(
            "/api/v1/egress-profiles",
            json!({
                "name": harness.unique_name("compose-project-egress-denied"),
                "description": "Compose e2e project policy denied egress",
                "labels": label_map("projects")
            }),
        )
        .await?;
    let disallowed_profile_id = json_id(&disallowed_profile, "id")?;

    let listed = harness.get_json("/api/v1/projects").await?;
    let listed_projects = json_array(&listed, "projects")?;
    if !listed_projects
        .iter()
        .any(|candidate| candidate.get("id") == Some(&json!(project_id)))
    {
        return Err(anyhow!("project list did not include {project_id}"));
    }

    let fetched = harness
        .get_json(&format!("/api/v1/projects/{project_id}"))
        .await?;
    if fetched["id"] != json!(project_id)
        || fetched["labels"]["suite"] != json!("bpane-gateway-compose-e2e")
        || fetched["labels"]["scope"] != json!("projects")
        || fetched["usage"]["max_active_sessions"] != json!(1)
    {
        return Err(anyhow!(
            "project get returned unexpected resource: {fetched}"
        ));
    }

    let template_name = harness.unique_name("compose-project-template");
    let template = harness
        .post_json(
            "/api/v1/session-templates",
            json!({
                "name": template_name,
                "defaults": {
                    "project_id": project_id,
                    "network_identity": {
                        "egress_profile_id": allowed_profile_id
                    },
                    "labels": {
                        "team": "support"
                    }
                }
            }),
        )
        .await?;
    let template_id = json_id(&template, "id")?;
    if template["defaults"]["project_id"] != json!(project_id) {
        return Err(anyhow!(
            "project template default was not persisted: {template}"
        ));
    }

    let disallowed_template = harness
        .post_json(
            "/api/v1/session-templates",
            json!({
                "name": harness.unique_name("compose-project-template-denied"),
                "defaults": {
                    "project_id": project_id,
                    "network_identity": {
                        "egress_profile_id": allowed_profile_id
                    }
                }
            }),
        )
        .await?;
    let disallowed_template_id = json_id(&disallowed_template, "id")?;

    let policy_project = harness
        .put_json(
            &format!("/api/v1/projects/{project_id}"),
            json!({
                "name": project_name,
                "description": "Compose e2e project with policy",
                "labels": label_map("projects"),
                "quotas": {
                    "max_active_sessions": 1,
                    "max_active_workflow_runs": 2,
                    "max_retained_storage_bytes": 1048576
                },
                "policy": {
                    "allowed_session_template_ids": [template_id],
                    "allowed_egress_profile_ids": [allowed_profile_id]
                }
            }),
        )
        .await?;
    if policy_project["policy"]["allowed_session_template_ids"][0] != json!(template_id)
        || policy_project["policy"]["allowed_egress_profile_ids"][0] != json!(allowed_profile_id)
    {
        return Err(anyhow!(
            "project policy update did not persist allow-lists: {policy_project}"
        ));
    }

    let template_policy_rejected = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "template_id": disallowed_template_id
            }),
        )
        .await?;
    if template_policy_rejected.status != StatusCode::CONFLICT
        || !template_policy_rejected.body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("session_template_not_allowed")
    {
        return Err(anyhow!(
            "project template policy rejection returned unexpected result {}: {}",
            template_policy_rejected.status,
            template_policy_rejected.body
        ));
    }

    let egress_policy_rejected = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "template_id": template_id,
                "network_identity": {
                    "egress_profile_id": disallowed_profile_id
                }
            }),
        )
        .await?;
    if egress_policy_rejected.status != StatusCode::CONFLICT
        || !egress_policy_rejected.body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("egress_profile_not_allowed")
    {
        return Err(anyhow!(
            "project egress policy rejection returned unexpected result {}: {}",
            egress_policy_rejected.status,
            egress_policy_rejected.body
        ));
    }

    let run_id = harness.unique_name("project-admission-e2e");
    let first = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "template_id": template_id,
                "labels": {
                    "run_id": run_id
                }
            }),
        )
        .await?;
    let first_session_id = json_id(&first, "id")?;
    if first["project_id"] != json!(project_id)
        || first["project"]["id"] != json!(project_id)
        || first["admission"]["state"] != json!("allowed")
        || first["admission"]["reason_code"] != json!("project_quota_available")
        || first["admission"]["active_sessions"] != json!(1)
    {
        return Err(anyhow!(
            "project-scoped session did not expose allowed admission: {first}"
        ));
    }

    let status = harness
        .get_json(&format!("/api/v1/sessions/{first_session_id}/status"))
        .await?;
    if status["project_id"] != json!(project_id) || status["admission"]["state"] != json!("allowed")
    {
        return Err(anyhow!(
            "project-scoped session status did not expose admission: {status}"
        ));
    }

    let rejected = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "template_id": template_id
            }),
        )
        .await?;
    if rejected.status != StatusCode::CONFLICT
        || !rejected.body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("active_session_quota_exceeded")
    {
        return Err(anyhow!(
            "project quota rejection returned unexpected result {}: {}",
            rejected.status,
            rejected.body
        ));
    }

    let usage = harness
        .get_json(&format!("/api/v1/projects/{project_id}/usage"))
        .await?;
    if usage["active_sessions"] != json!(1) {
        return Err(anyhow!(
            "project usage did not count the active session: {usage}"
        ));
    }

    let stopped_first = harness
        .delete_json(&format!("/api/v1/sessions/{first_session_id}"))
        .await?;
    if stopped_first["state"] != json!("stopped") {
        return Err(anyhow!(
            "first project-scoped session cleanup did not stop the session"
        ));
    }

    let second = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "template_id": template_id,
                "labels": {
                    "run_id": run_id,
                    "retry": "true"
                }
            }),
        )
        .await?;
    let second_session_id = json_id(&second, "id")?;
    if second["admission"]["state"] != json!("allowed") {
        return Err(anyhow!(
            "project admission did not recover after stopping first session: {second}"
        ));
    }

    let archived = harness
        .put_json(
            &format!("/api/v1/projects/{project_id}"),
            json!({
                "name": project_name,
                "description": "Compose e2e project archived",
                "labels": label_map("projects"),
                "quotas": {
                    "max_active_sessions": 1,
                    "max_active_workflow_runs": 2,
                    "max_retained_storage_bytes": 1048576
                },
                "state": "archived"
            }),
        )
        .await?;
    if archived["state"] != json!("archived") {
        return Err(anyhow!("project archive did not persist: {archived}"));
    }

    let archived_rejected = harness
        .post_json_outcome(
            "/api/v1/sessions",
            json!({
                "project_id": project_id
            }),
        )
        .await?;
    if archived_rejected.status != StatusCode::CONFLICT
        || !archived_rejected.body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("project_archived")
    {
        return Err(anyhow!(
            "archived project rejection returned unexpected result {}: {}",
            archived_rejected.status,
            archived_rejected.body
        ));
    }

    let stopped_second = harness
        .delete_json(&format!("/api/v1/sessions/{second_session_id}"))
        .await?;
    if stopped_second["state"] != json!("stopped") {
        return Err(anyhow!(
            "second project-scoped session cleanup did not stop the session"
        ));
    }

    Ok(())
}
