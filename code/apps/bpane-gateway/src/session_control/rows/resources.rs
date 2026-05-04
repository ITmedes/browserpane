use super::super::*;
use super::encoding::row_to_json_string_array;

pub(in crate::session_control) fn row_to_stored_credential_binding(
    row: &Row,
) -> Result<StoredCredentialBinding, SessionStoreError> {
    let provider = row
        .get::<_, String>("provider")
        .parse::<CredentialBindingProvider>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let injection_mode = row
        .get::<_, String>("injection_mode")
        .parse::<CredentialInjectionMode>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("credential binding labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("credential binding label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;
    let allowed_origins = row_to_json_string_array(
        row.get("allowed_origins"),
        "credential binding allowed_origins",
    )?;
    let totp = row
        .get::<_, Option<Value>>("totp")
        .map(|value| {
            serde_json::from_value::<CredentialTotpMetadata>(value).map_err(|error| {
                SessionStoreError::Backend(format!(
                    "credential binding totp column must be valid totp json: {error}"
                ))
            })
        })
        .transpose()?;

    Ok(StoredCredentialBinding {
        id: row.get("id"),
        owner_subject: row.get("owner_subject"),
        owner_issuer: row.get("owner_issuer"),
        name: row.get("name"),
        provider,
        external_ref: row.get("external_ref"),
        namespace: row.get("namespace"),
        allowed_origins,
        injection_mode,
        totp,
        labels,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(in crate::session_control) fn row_to_stored_extension_definition(
    row: &Row,
) -> Result<StoredExtensionDefinition, SessionStoreError> {
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("extension labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("extension label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;

    Ok(StoredExtensionDefinition {
        id: row.get("id"),
        owner_subject: row.get("owner_subject"),
        owner_issuer: row.get("owner_issuer"),
        name: row.get("name"),
        description: row.get("description"),
        enabled: row.get("enabled"),
        latest_version: row.get("latest_version"),
        labels,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(in crate::session_control) fn row_to_stored_extension_version(
    row: &Row,
) -> Result<StoredExtensionVersion, SessionStoreError> {
    Ok(StoredExtensionVersion {
        id: row.get("id"),
        extension_definition_id: row.get("extension_definition_id"),
        version: row.get("version"),
        install_path: row.get("install_path"),
        created_at: row.get("created_at"),
    })
}

pub(in crate::session_control) fn row_to_stored_workflow_definition(
    row: &Row,
) -> Result<StoredWorkflowDefinition, SessionStoreError> {
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("workflow definition labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("workflow definition label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;

    Ok(StoredWorkflowDefinition {
        id: row.get("id"),
        owner_subject: row.get("owner_subject"),
        owner_issuer: row.get("owner_issuer"),
        name: row.get("name"),
        description: row.get("description"),
        labels,
        latest_version: row.get("latest_version"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(in crate::session_control) fn row_to_stored_workflow_definition_version(
    row: &Row,
) -> Result<StoredWorkflowDefinitionVersion, SessionStoreError> {
    let source = row
        .get::<_, Option<Value>>("source")
        .map(|value| {
            serde_json::from_value::<WorkflowSource>(value).map_err(|error| {
                SessionStoreError::Backend(format!(
                    "workflow definition version source must be valid workflow source json: {error}"
                ))
            })
        })
        .transpose()?;
    let allowed_credential_binding_ids = row_to_json_string_array(
        row.get("allowed_credential_binding_ids"),
        "workflow allowed_credential_binding_ids",
    )?;
    let allowed_extension_ids = row_to_json_string_array(
        row.get("allowed_extension_ids"),
        "workflow allowed_extension_ids",
    )?;
    let allowed_file_workspace_ids = row_to_json_string_array(
        row.get("allowed_file_workspace_ids"),
        "workflow allowed_file_workspace_ids",
    )?;

    Ok(StoredWorkflowDefinitionVersion {
        id: row.get("id"),
        workflow_definition_id: row.get("workflow_definition_id"),
        version: row.get("version"),
        executor: row.get("executor"),
        entrypoint: row.get("entrypoint"),
        source,
        input_schema: row.get("input_schema"),
        output_schema: row.get("output_schema"),
        default_session: row.get("default_session"),
        allowed_credential_binding_ids,
        allowed_extension_ids,
        allowed_file_workspace_ids,
        created_at: row.get("created_at"),
    })
}

pub(in crate::session_control) fn row_to_stored_file_workspace(
    row: &Row,
) -> Result<StoredFileWorkspace, SessionStoreError> {
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("file workspace labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("file workspace label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;

    Ok(StoredFileWorkspace {
        id: row.get("id"),
        owner_subject: row.get("owner_subject"),
        owner_issuer: row.get("owner_issuer"),
        name: row.get("name"),
        description: row.get("description"),
        labels,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(in crate::session_control) fn row_to_stored_file_workspace_file(
    row: &Row,
) -> Result<StoredFileWorkspaceFile, SessionStoreError> {
    let byte_count = u64::try_from(row.get::<_, i64>("byte_count")).map_err(|error| {
        SessionStoreError::Backend(format!(
            "workspace file byte_count must be non-negative and fit u64: {error}"
        ))
    })?;
    let provenance: Option<Value> = row.get("provenance");
    if provenance.as_ref().is_some_and(|value| !value.is_object()) {
        return Err(SessionStoreError::Backend(
            "workspace file provenance column must be a JSON object".to_string(),
        ));
    }

    Ok(StoredFileWorkspaceFile {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        name: row.get("name"),
        media_type: row.get("media_type"),
        byte_count,
        sha256_hex: row.get("sha256_hex"),
        provenance,
        artifact_ref: row.get("artifact_ref"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(in crate::session_control) fn row_to_stored_session_file_binding(
    row: &Row,
) -> Result<StoredSessionFileBinding, SessionStoreError> {
    let byte_count = u64::try_from(row.get::<_, i64>("byte_count")).map_err(|error| {
        SessionStoreError::Backend(format!(
            "session file binding byte_count must be non-negative and fit u64: {error}"
        ))
    })?;
    let provenance: Option<Value> = row.get("provenance");
    if provenance.as_ref().is_some_and(|value| !value.is_object()) {
        return Err(SessionStoreError::Backend(
            "session file binding provenance column must be a JSON object".to_string(),
        ));
    }
    let labels_value: Value = row.get("labels");
    let labels = labels_value
        .as_object()
        .context("session file binding labels column must be a JSON object")
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value
                    .as_str()
                    .context("session file binding label values must be strings")
                    .map_err(|error| SessionStoreError::Backend(error.to_string()))?
                    .to_string(),
            ))
        })
        .collect::<Result<HashMap<_, _>, SessionStoreError>>()?;
    let mode = row
        .get::<_, String>("mode")
        .parse::<SessionFileBindingMode>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let state = row
        .get::<_, String>("state")
        .parse::<SessionFileBindingState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;

    Ok(StoredSessionFileBinding {
        id: row.get("id"),
        session_id: row.get("session_id"),
        workspace_id: row.get("workspace_id"),
        file_id: row.get("file_id"),
        file_name: row.get("file_name"),
        media_type: row.get("media_type"),
        byte_count,
        sha256_hex: row.get("sha256_hex"),
        provenance,
        artifact_ref: row.get("artifact_ref"),
        mount_path: row.get("mount_path"),
        mode,
        state,
        error: row.get("error"),
        labels,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
