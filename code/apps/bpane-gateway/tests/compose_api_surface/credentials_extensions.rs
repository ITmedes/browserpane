use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::json;

use super::support::{json_array, json_id, label_map, recording_policy, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    harness.ensure_workflow_worker_image().await?;
    let source = harness.create_local_workflow_repo().await?;

    let binding = harness
        .post_json(
            "/api/v1/credential-bindings",
            json!({
                "name": harness.unique_name("compose-e2e-login"),
                "provider": "vault_kv_v2",
                "namespace": "compose-e2e",
                "allowed_origins": ["http://web:8080"],
                "injection_mode": "form_fill",
                "secret_payload": {
                    "username": "demo",
                    "password": "demo-demo",
                },
                "labels": label_map("credentials"),
            }),
        )
        .await?;
    let binding_id = json_id(&binding, "id")?;
    if binding.get("secret_payload").is_some() {
        return Err(anyhow!("credential binding response leaked secret payload"));
    }

    let listed_bindings = harness.get_json("/api/v1/credential-bindings").await?;
    let listed_bindings = json_array(&listed_bindings, "credential_bindings")?;
    if !listed_bindings
        .iter()
        .any(|candidate| candidate.get("id") == Some(&json!(binding_id)))
    {
        return Err(anyhow!(
            "credential binding {binding_id} missing from list endpoint"
        ));
    }

    let fetched_binding = harness
        .get_json(&format!("/api/v1/credential-bindings/{binding_id}"))
        .await?;
    if fetched_binding["id"] != json!(binding_id) {
        return Err(anyhow!(
            "credential binding lookup returned the wrong resource"
        ));
    }
    if fetched_binding.get("secret_payload").is_some() {
        return Err(anyhow!("credential binding fetch leaked secret payload"));
    }

    let extension = harness
        .post_json(
            "/api/v1/extensions",
            json!({
                "name": harness.unique_name("compose-e2e-extension"),
                "description": "Compose-backed extension coverage",
                "labels": label_map("extensions"),
            }),
        )
        .await?;
    let extension_id = json_id(&extension, "id")?;
    if extension["enabled"] != json!(true) {
        return Err(anyhow!("new extension did not start enabled"));
    }

    let listed_extensions = harness.get_json("/api/v1/extensions").await?;
    let listed_extensions = json_array(&listed_extensions, "extensions")?;
    if !listed_extensions
        .iter()
        .any(|candidate| candidate.get("id") == Some(&json!(extension_id)))
    {
        return Err(anyhow!(
            "extension {extension_id} missing from list endpoint"
        ));
    }

    let fetched_extension = harness
        .get_json(&format!("/api/v1/extensions/{extension_id}"))
        .await?;
    if fetched_extension["id"] != json!(extension_id) {
        return Err(anyhow!("extension lookup returned the wrong resource"));
    }
    if fetched_extension["enabled"] != json!(true) {
        return Err(anyhow!(
            "extension fetch did not reflect enabled default state"
        ));
    }

    let extension_version = harness
        .post_json(
            &format!("/api/v1/extensions/{extension_id}/versions"),
            json!({
                "version": "1.0.0",
                "install_path": "/home/bpane/bpane-test-extension",
            }),
        )
        .await?;
    let _extension_version_id = json_id(&extension_version, "id")?;
    if extension_version["extension_definition_id"] != json!(extension_id) {
        return Err(anyhow!(
            "extension version did not link back to the created extension"
        ));
    }

    let disabled_extension = harness
        .post_json(
            &format!("/api/v1/extensions/{extension_id}/disable"),
            json!({}),
        )
        .await?;
    if disabled_extension["enabled"] != json!(false) {
        return Err(anyhow!(
            "extension disable endpoint did not disable the extension"
        ));
    }

    let disabled_lookup = harness
        .get_json(&format!("/api/v1/extensions/{extension_id}"))
        .await?;
    if disabled_lookup["enabled"] != json!(false) {
        return Err(anyhow!("extension lookup did not reflect disabled state"));
    }

    let enabled_extension = harness
        .post_json(
            &format!("/api/v1/extensions/{extension_id}/enable"),
            json!({}),
        )
        .await?;
    if enabled_extension["enabled"] != json!(true) {
        return Err(anyhow!(
            "extension enable endpoint did not enable the extension"
        ));
    }
    if enabled_extension["latest_version"] != json!("1.0.0") {
        return Err(anyhow!("enabled extension did not expose latest version"));
    }

    let output_workspace = harness
        .post_json(
            "/api/v1/file-workspaces",
            json!({
                "name": harness.unique_name("compose-e2e-credential-output"),
                "description": "Compose-backed workflow credential outputs",
                "labels": label_map("credentials-output-workspace"),
            }),
        )
        .await?;
    let output_workspace_id = json_id(&output_workspace, "id")?;

    let workflow = harness
        .post_json(
            "/api/v1/workflows",
            json!({
                "name": harness.unique_name("compose-e2e-credential-workflow"),
                "description": "Compose-backed workflow credential resolution",
                "labels": label_map("credentials-workflow"),
            }),
        )
        .await?;
    let workflow_id = json_id(&workflow, "id")?;

    let workflow_version = harness
        .post_json(
            &format!("/api/v1/workflows/{workflow_id}/versions"),
            json!({
                "version": "v1",
                "executor": "playwright",
                "entrypoint": "workflows/smoke/run.mjs",
                "source": {
                    "kind": "git",
                    "repository_url": source.repository_url,
                    "ref": "refs/heads/main",
                    "root_path": "workflows",
                },
                "input_schema": {
                    "type": "object",
                    "required": ["target_url", "output_workspace_id"],
                },
                "output_schema": {
                    "type": "object",
                    "required": ["title", "final_url", "output_file_name"],
                },
                "default_session": {
                    "labels": label_map("credentials-default-session"),
                    "recording": recording_policy("manual"),
                },
                "allowed_file_workspace_ids": [output_workspace_id],
                "allowed_credential_binding_ids": [binding_id],
            }),
        )
        .await?;
    if workflow_version["source"]["resolved_commit"] != json!(source.commit) {
        return Err(anyhow!(
            "credential workflow version did not resolve the expected commit"
        ));
    }

    let run = harness
        .post_json(
            "/api/v1/workflow-runs",
            json!({
                "workflow_id": workflow_id,
                "version": "v1",
                "client_request_id": harness.unique_name("compose-e2e-credential-run"),
                "source_system": "bpane-gateway-compose-e2e",
                "source_reference": "credential-resolution",
                "input": {
                    "target_url": "http://web:8080",
                    "output_workspace_id": output_workspace_id,
                },
                "credential_binding_ids": [binding_id],
                "labels": label_map("credentials-run"),
            }),
        )
        .await?;
    let run_id = json_id(&run, "id")?;
    let run_session_id = json_id(&run, "session_id")?;

    let run_credential_bindings = json_array(&run, "credential_bindings")?;
    if run_credential_bindings.len() != 1 {
        return Err(anyhow!(
            "workflow run did not expose the requested credential binding"
        ));
    }
    if run_credential_bindings[0]["id"] != json!(binding_id) {
        return Err(anyhow!("workflow run bound the wrong credential binding"));
    }

    let automation_access = harness
        .post_json(
            &format!("/api/v1/sessions/{run_session_id}/automation-access"),
            json!({}),
        )
        .await?;
    let automation_token = json_id(&automation_access, "token")?;

    let resolved = harness
        .get_json_with_automation_token(
            &format!("/api/v1/workflow-runs/{run_id}/credential-bindings/{binding_id}/resolved"),
            &automation_token,
        )
        .await?;
    if resolved["binding"]["id"] != json!(binding_id) {
        return Err(anyhow!(
            "resolved workflow credential binding returned the wrong binding resource"
        ));
    }
    if resolved["payload"]
        != json!({
            "username": "demo",
            "password": "demo-demo",
        })
    {
        return Err(anyhow!(
            "resolved workflow credential binding returned the wrong secret payload"
        ));
    }

    let completed_run = harness
        .poll_json(
            "credential workflow run completion",
            Duration::from_secs(60),
            |value| value["state"] == json!("succeeded"),
            &format!("/api/v1/workflow-runs/{run_id}"),
        )
        .await?;
    if completed_run["state"] != json!("succeeded") {
        return Err(anyhow!(
            "credential workflow run did not complete successfully"
        ));
    }

    let events = harness
        .get_json(&format!("/api/v1/workflow-runs/{run_id}/events"))
        .await?;
    let events = json_array(&events, "events")?;
    if !events
        .iter()
        .any(|event| event["event_type"] == json!("workflow_run.credential_binding_resolved"))
    {
        return Err(anyhow!(
            "workflow run events did not record credential binding resolution"
        ));
    }

    let _deleted_run_session = harness
        .delete_json(&format!("/api/v1/sessions/{run_session_id}"))
        .await?;

    Ok(())
}
