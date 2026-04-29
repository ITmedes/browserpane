use anyhow::{anyhow, Result};
use serde_json::json;

use super::support::{json_array, json_id, label_map, map_headers, ComposeHarness};

pub async fn run(harness: &ComposeHarness) -> Result<()> {
    let workspace = harness
        .post_json(
            "/api/v1/file-workspaces",
            json!({
                "name": harness.unique_name("compose-e2e-workspace"),
                "description": "Compose-backed gateway file workspace e2e",
                "labels": label_map("workspaces-automation"),
            }),
        )
        .await?;
    let workspace_id = json_id(&workspace, "id")?;

    let workspaces = harness.get_json("/api/v1/file-workspaces").await?;
    let workspaces = json_array(&workspaces, "workspaces")?;
    if !workspaces
        .iter()
        .any(|candidate| candidate.get("id") == Some(&json!(workspace_id)))
    {
        return Err(anyhow!(
            "workspace {workspace_id} missing from list endpoint"
        ));
    }

    let fetched_workspace = harness
        .get_json(&format!("/api/v1/file-workspaces/{workspace_id}"))
        .await?;
    if fetched_workspace["id"] != json!(workspace_id) {
        return Err(anyhow!("workspace lookup returned the wrong resource"));
    }

    let file = harness
        .post_bytes(
            &format!("/api/v1/file-workspaces/{workspace_id}/files"),
            b"compose gateway e2e workspace bytes".to_vec(),
            "text/plain; charset=utf-8",
            &[("x-bpane-file-name", "compose-e2e.txt")],
        )
        .await?;
    let file_id = json_id(&file, "id")?;

    let files = harness
        .get_json(&format!("/api/v1/file-workspaces/{workspace_id}/files"))
        .await?;
    let files = json_array(&files, "files")?;
    if files.is_empty() {
        return Err(anyhow!("workspace file list is empty after upload"));
    }

    let fetched_file = harness
        .get_json(&format!(
            "/api/v1/file-workspaces/{workspace_id}/files/{file_id}"
        ))
        .await?;
    if fetched_file["name"] != json!("compose-e2e.txt") {
        return Err(anyhow!("workspace file metadata did not round-trip"));
    }

    let file_bytes = harness
        .get_bytes(&format!(
            "/api/v1/file-workspaces/{workspace_id}/files/{file_id}/content"
        ))
        .await?;
    if file_bytes != b"compose gateway e2e workspace bytes" {
        return Err(anyhow!(
            "workspace file content endpoint returned unexpected bytes"
        ));
    }

    let session = harness
        .post_json(
            "/api/v1/sessions",
            json!({
                "labels": label_map("workspaces-automation-session"),
            }),
        )
        .await?;
    let session_id = json_id(&session, "id")?;

    let task = harness
        .post_json(
            "/api/v1/automation-tasks",
            json!({
                "display_name": "Compose E2E automation task",
                "executor": "compose_e2e",
                "session": {
                    "existing_session_id": session_id,
                },
                "input": {
                    "workspace_id": workspace_id,
                    "file_id": file_id,
                },
                "labels": label_map("workspaces-automation-task"),
            }),
        )
        .await?;
    let task_id = json_id(&task, "id")?;

    let listed_tasks = harness.get_json("/api/v1/automation-tasks").await?;
    let listed_tasks = json_array(&listed_tasks, "tasks")?;
    if !listed_tasks
        .iter()
        .any(|candidate| candidate.get("id") == Some(&json!(task_id)))
    {
        return Err(anyhow!(
            "automation task {task_id} missing from list endpoint"
        ));
    }

    let fetched_task = harness
        .get_json(&format!("/api/v1/automation-tasks/{task_id}"))
        .await?;
    if fetched_task["executor"] != json!("compose_e2e") {
        return Err(anyhow!(
            "automation task lookup returned the wrong executor"
        ));
    }

    let automation_access = harness
        .post_json(
            &format!("/api/v1/sessions/{session_id}/automation-access"),
            json!({}),
        )
        .await?;
    let automation_token = json_id(&automation_access, "token")?;

    let automation_headers =
        map_headers(&[("x-bpane-automation-access-token", &automation_token)])?;

    let running = harness
        .post_json_with_headers(
            &format!("/api/v1/automation-tasks/{task_id}/state"),
            json!({
                "state": "running",
                "message": "compose e2e running",
                "data": {
                    "phase": "running",
                },
            }),
            automation_headers.clone(),
        )
        .await?;
    if running["state"] != json!("running") {
        return Err(anyhow!("automation task did not transition to running"));
    }

    let _log = harness
        .post_json_with_headers(
            &format!("/api/v1/automation-tasks/{task_id}/logs"),
            json!({
                "stream": "stdout",
                "message": "compose automation log line",
            }),
            automation_headers.clone(),
        )
        .await?;

    let succeeded = harness
        .post_json_with_headers(
            &format!("/api/v1/automation-tasks/{task_id}/state"),
            json!({
                "state": "succeeded",
                "message": "compose e2e complete",
                "output": {
                    "ok": true,
                },
            }),
            automation_headers,
        )
        .await?;
    if succeeded["state"] != json!("succeeded") {
        return Err(anyhow!("automation task did not transition to succeeded"));
    }

    let task_events = harness
        .get_json(&format!("/api/v1/automation-tasks/{task_id}/events"))
        .await?;
    let task_events = json_array(&task_events, "events")?;
    if task_events.len() < 2 {
        return Err(anyhow!(
            "automation task events did not capture state transitions"
        ));
    }

    let task_logs = harness
        .get_json(&format!("/api/v1/automation-tasks/{task_id}/logs"))
        .await?;
    let task_logs = json_array(&task_logs, "logs")?;
    if task_logs.is_empty() {
        return Err(anyhow!(
            "automation task logs did not capture automation output"
        ));
    }

    let cancelled_task = harness
        .post_json(
            "/api/v1/automation-tasks",
            json!({
                "display_name": "Compose E2E automation cancel task",
                "executor": "compose_e2e_cancel",
                "session": {
                    "existing_session_id": session_id,
                },
                "labels": label_map("workspaces-automation-cancel"),
            }),
        )
        .await?;
    let cancelled_task_id = json_id(&cancelled_task, "id")?;
    let cancelled = harness
        .post_json(
            &format!("/api/v1/automation-tasks/{cancelled_task_id}/cancel"),
            json!({}),
        )
        .await?;
    if cancelled["state"] != json!("cancelled") {
        return Err(anyhow!(
            "automation task cancel endpoint did not cancel the task"
        ));
    }

    let deleted_file = harness
        .delete_json(&format!(
            "/api/v1/file-workspaces/{workspace_id}/files/{file_id}"
        ))
        .await?;
    if deleted_file["id"] != json!(file_id) {
        return Err(anyhow!("workspace file delete returned the wrong file"));
    }

    let _deleted_session = harness
        .delete_json(&format!("/api/v1/sessions/{session_id}"))
        .await?;

    let refreshed_workspaces = harness.get_json("/api/v1/file-workspaces").await?;
    let refreshed_workspaces = json_array(&refreshed_workspaces, "workspaces")?;
    if !refreshed_workspaces
        .iter()
        .any(|candidate| candidate.get("id") == Some(&json!(workspace_id)))
    {
        return Err(anyhow!(
            "workspace resource disappeared unexpectedly after file deletion"
        ));
    }

    Ok(())
}
