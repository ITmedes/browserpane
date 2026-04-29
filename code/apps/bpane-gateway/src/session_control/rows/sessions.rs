use super::super::*;

pub(in crate::session_control) fn row_to_stored_session(
    row: &Row,
) -> Result<StoredSession, SessionStoreError> {
    let state = row
        .get::<_, String>("state")
        .parse::<SessionLifecycleState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let owner_mode = row
        .get::<_, String>("owner_mode")
        .parse::<SessionOwnerMode>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;
    let recording = serde_json::from_value::<SessionRecordingPolicy>(row.get("recording"))
        .map_err(|error| {
            SessionStoreError::Backend(format!("failed to decode recording policy: {error}"))
        })?;
    let extensions = serde_json::from_value::<Vec<AppliedExtension>>(row.get("extensions"))
        .map_err(|error| {
            SessionStoreError::Backend(format!("failed to decode session extensions: {error}"))
        })?;

    let width = row.get::<_, i32>("viewport_width");
    let height = row.get::<_, i32>("viewport_height");
    let automation_owner_client_id = row.get::<_, Option<String>>("automation_owner_client_id");
    let automation_owner_issuer = row.get::<_, Option<String>>("automation_owner_issuer");

    Ok(StoredSession {
        id: row.get("id"),
        state,
        template_id: row.get("template_id"),
        owner_mode,
        viewport: SessionViewport {
            width: width as u16,
            height: height as u16,
        },
        owner: SessionOwner {
            subject: row.get("owner_subject"),
            issuer: row.get("owner_issuer"),
            display_name: row.get("owner_display_name"),
        },
        automation_delegate: match (automation_owner_client_id, automation_owner_issuer) {
            (Some(client_id), Some(issuer)) => Some(SessionAutomationDelegate {
                client_id,
                issuer,
                display_name: row.get("automation_owner_display_name"),
            }),
            _ => None,
        },
        idle_timeout_sec: row
            .get::<_, Option<i32>>("idle_timeout_sec")
            .map(|value| value as u32),
        labels,
        integration_context: row.get("integration_context"),
        extensions,
        recording,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        stopped_at: row.get("stopped_at"),
    })
}

pub(in crate::session_control) fn row_to_stored_session_recording(
    row: &Row,
) -> Result<StoredSessionRecording, SessionStoreError> {
    let state = row
        .get::<_, String>("state")
        .parse::<SessionRecordingState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let format = row
        .get::<_, String>("format")
        .parse::<SessionRecordingFormat>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let termination_reason = row
        .get::<_, Option<String>>("termination_reason")
        .map(|value| {
            value
                .parse::<SessionRecordingTerminationReason>()
                .map_err(|error| SessionStoreError::Backend(error.to_string()))
        })
        .transpose()?;

    Ok(StoredSessionRecording {
        id: row.get("id"),
        session_id: row.get("session_id"),
        previous_recording_id: row.get("previous_recording_id"),
        state,
        format,
        mime_type: row.get("mime_type"),
        bytes: row
            .get::<_, Option<i64>>("byte_count")
            .map(|value| value as u64),
        duration_ms: row
            .get::<_, Option<i64>>("duration_ms")
            .map(|value| value as u64),
        error: row.get("error"),
        termination_reason,
        artifact_ref: row.get("artifact_ref"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
