use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Write};

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;

use super::*;
use crate::session_control::{BrowserContextUsageResource, StoredBrowserContext};

pub(super) fn browser_context_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/browser-contexts",
            post(create_browser_context).get(list_browser_contexts),
        )
        .route(
            "/api/v1/browser-contexts/import",
            post(import_browser_context).layer(DefaultBodyLimit::disable()),
        )
        .route(
            "/api/v1/browser-contexts/{context_id}/clone",
            post(clone_browser_context),
        )
        .route(
            "/api/v1/browser-contexts/{context_id}/export",
            get(export_browser_context),
        )
        .route(
            "/api/v1/browser-contexts/{context_id}",
            get(get_browser_context).delete(delete_browser_context),
        )
}

async fn list_browser_contexts(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<BrowserContextListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let contexts = state
        .session_store
        .list_browser_contexts_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let contexts = browser_context_resources_with_usage(&state, &principal, contexts).await?;
    Ok(Json(BrowserContextListResponse { contexts }))
}

async fn create_browser_context(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateBrowserContextRequest>,
) -> Result<(StatusCode, Json<BrowserContextResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let context = state
        .session_store
        .create_browser_context(
            &principal,
            PersistBrowserContextRequest {
                id: None,
                project_id: request.project_id,
                name: request.name,
                description: request.description,
                labels: request.labels,
                persistence_mode: request.persistence_mode,
                retention_sec: request.retention_sec,
                max_profile_storage_bytes: request.max_profile_storage_bytes,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((
        StatusCode::CREATED,
        Json(browser_context_resource_with_usage(&state, &principal, context).await?),
    ))
}

async fn import_browser_context(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    body: Bytes,
) -> Result<(StatusCode, Json<BrowserContextResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let archive = parse_browser_context_import_archive(body.as_ref()).map_err(bad_request)?;
    let target_context_id = Uuid::now_v7();
    let target_request = browser_context_import_request_from_headers(
        &headers,
        target_context_id,
        &archive.manifest,
    )?;
    SessionStore::validate_browser_context_request(&target_request)
        .map_err(map_session_store_error)?;

    state
        .session_manager
        .import_browser_context_profile_archive(
            target_context_id,
            archive.profile_archive.as_deref(),
        )
        .await
        .map_err(map_browser_context_import_runtime_error)?;

    let context = match state
        .session_store
        .create_browser_context(&principal, target_request)
        .await
    {
        Ok(context) => context,
        Err(error) => {
            if let Err(cleanup_error) = state
                .session_manager
                .delete_browser_context_data(target_context_id)
                .await
            {
                warn!(
                    target_browser_context_id = %target_context_id,
                    error = %cleanup_error,
                    "failed to clean up imported browser context data after metadata persistence failure",
                );
            }
            return Err(map_session_store_error(error));
        }
    };

    Ok((
        StatusCode::CREATED,
        Json(browser_context_resource_with_usage(&state, &principal, context).await?),
    ))
}

async fn clone_browser_context(
    headers: HeaderMap,
    Path(context_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CloneBrowserContextRequest>,
) -> Result<(StatusCode, Json<BrowserContextResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let source = state
        .session_store
        .get_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    if source.state != BrowserContextState::Ready {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("browser context {context_id} is deleted and cannot be cloned"),
            }),
        ));
    }
    if source.persistence_mode != BrowserContextPersistenceMode::Reusable {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("browser context {context_id} is not reusable and cannot be cloned"),
            }),
        ));
    }
    if let Some(active_session_id) = state
        .session_manager
        .active_browser_context_session_id(context_id)
        .await
    {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "browser context {context_id} is already used by active session {active_session_id}"
                ),
            }),
        ));
    }

    let target_context_id = Uuid::now_v7();
    let target_request = PersistBrowserContextRequest {
        id: Some(target_context_id),
        project_id: request.project_id.or(source.project_id),
        name: request.name,
        description: request.description.or_else(|| source.description.clone()),
        labels: request.labels.unwrap_or_else(|| source.labels.clone()),
        persistence_mode: BrowserContextPersistenceMode::Reusable,
        retention_sec: request.retention_sec.or(source.retention_sec),
        max_profile_storage_bytes: request
            .max_profile_storage_bytes
            .or(source.max_profile_storage_bytes),
    };
    SessionStore::validate_browser_context_request(&target_request)
        .map_err(map_session_store_error)?;

    state
        .session_manager
        .clone_browser_context_data(source.id, target_context_id)
        .await
        .map_err(|error| {
            (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: error.to_string(),
                }),
            )
        })?;

    let context = match state
        .session_store
        .create_browser_context(&principal, target_request)
        .await
    {
        Ok(context) => context,
        Err(error) => {
            if let Err(cleanup_error) = state
                .session_manager
                .delete_browser_context_data(target_context_id)
                .await
            {
                warn!(
                    source_browser_context_id = %source.id,
                    target_browser_context_id = %target_context_id,
                    error = %cleanup_error,
                    "failed to clean up cloned browser context data after metadata persistence failure",
                );
            }
            return Err(map_session_store_error(error));
        }
    };

    Ok((
        StatusCode::CREATED,
        Json(browser_context_resource_with_usage(&state, &principal, context).await?),
    ))
}

async fn export_browser_context(
    headers: HeaderMap,
    Path(context_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let context = state
        .session_store
        .get_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    if context.state != BrowserContextState::Ready {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("browser context {context_id} is deleted and cannot be exported"),
            }),
        ));
    }
    if context.persistence_mode != BrowserContextPersistenceMode::Reusable {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "browser context {context_id} is not reusable and cannot be exported"
                ),
            }),
        ));
    }
    if let Some(active_session_id) = state
        .session_manager
        .active_browser_context_session_id(context_id)
        .await
    {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "browser context {context_id} is already used by active session {active_session_id}"
                ),
            }),
        ));
    }

    let resource = browser_context_resource_with_usage(&state, &principal, context.clone()).await?;
    let profile_archive = state
        .session_manager
        .export_browser_context_profile_archive(context.id)
        .await
        .map_err(|error| {
            (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: error.to_string(),
                }),
            )
        })?;
    let manifest = BrowserContextExportManifest {
        format_version: 1,
        archive_type: "browser_context_export".to_string(),
        exported_at: Utc::now(),
        source_context: resource,
        profile_archive_path: profile_archive
            .as_ref()
            .map(|_| BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH.to_string()),
    };
    let bytes = build_browser_context_export_archive(&manifest, profile_archive.as_deref())
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to build browser context export archive: {error}"),
                }),
            )
        })?;
    let filename = format!("browserpane-browser-context-{context_id}.zip");
    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string()).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to encode content length header: {error}"),
                }),
            )
        })?,
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).map_err(
            |error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to encode content disposition header: {error}"),
                    }),
                )
            },
        )?,
    );
    Ok(response)
}

async fn get_browser_context(
    headers: HeaderMap,
    Path(context_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<BrowserContextResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let context = state
        .session_store
        .get_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    Ok(Json(
        browser_context_resource_with_usage(&state, &principal, context).await?,
    ))
}

async fn delete_browser_context(
    headers: HeaderMap,
    Path(context_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<BrowserContextResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let existing = state
        .session_store
        .get_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    if existing.state != BrowserContextState::Deleted {
        state
            .session_manager
            .delete_browser_context_data(context_id)
            .await
            .map_err(|error| {
                (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: error.to_string(),
                    }),
                )
            })?;
    }
    let context = state
        .session_store
        .delete_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    Ok(Json(
        browser_context_resource_with_usage(&state, &principal, context).await?,
    ))
}

async fn browser_context_resources_with_usage(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    contexts: Vec<StoredBrowserContext>,
) -> Result<Vec<BrowserContextResource>, (StatusCode, Json<ErrorResponse>)> {
    let context_ids = contexts
        .iter()
        .map(|context| context.id)
        .collect::<Vec<_>>();
    let usage_by_context = browser_context_usage_by_id(state, principal, &context_ids).await?;
    let mut resources = Vec::with_capacity(contexts.len());
    for context in contexts {
        let usage = usage_by_context
            .get(&context.id)
            .cloned()
            .unwrap_or_default();
        let project = browser_context_project_summary(state, principal, context.project_id).await?;
        resources.push(browser_context_resource_with_usage_value(
            context, usage, project,
        ));
    }
    Ok(resources)
}

async fn browser_context_resource_with_usage(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    context: StoredBrowserContext,
) -> Result<BrowserContextResource, (StatusCode, Json<ErrorResponse>)> {
    let mut usage_by_context = browser_context_usage_by_id(state, principal, &[context.id]).await?;
    let usage = usage_by_context.remove(&context.id).unwrap_or_default();
    let project = browser_context_project_summary(state, principal, context.project_id).await?;
    Ok(browser_context_resource_with_usage_value(
        context, usage, project,
    ))
}

fn browser_context_resource_with_usage_value(
    context: StoredBrowserContext,
    mut usage: BrowserContextUsageResource,
    project: Option<SessionProjectResource>,
) -> BrowserContextResource {
    let mut resource = context.to_resource();
    usage.profile_storage_limit_exceeded = match (
        resource.max_profile_storage_bytes,
        usage.profile_storage_bytes,
    ) {
        (Some(limit), Some(bytes)) => bytes > limit,
        _ => false,
    };
    resource.usage = usage;
    resource.project = project;
    resource
}

async fn browser_context_project_summary(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    project_id: Option<Uuid>,
) -> Result<Option<SessionProjectResource>, (StatusCode, Json<ErrorResponse>)> {
    let Some(project_id) = project_id else {
        return Ok(None);
    };
    Ok(state
        .session_store
        .get_project_for_owner(principal, project_id)
        .await
        .map_err(map_session_store_error)?
        .map(|project| project.to_session_project_resource()))
}

const BROWSER_CONTEXT_EXPORT_MANIFEST_PATH: &str = "manifest.json";
const BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH: &str = "profile.tar.gz";
const MAX_BROWSER_CONTEXT_EXPORT_MANIFEST_BYTES: u64 = 128 * 1024;

#[derive(Deserialize, Serialize)]
struct BrowserContextExportManifest {
    format_version: u32,
    archive_type: String,
    exported_at: chrono::DateTime<Utc>,
    source_context: BrowserContextResource,
    profile_archive_path: Option<String>,
}

struct ParsedBrowserContextImportArchive {
    manifest: BrowserContextExportManifest,
    profile_archive: Option<Vec<u8>>,
}

fn build_browser_context_export_archive(
    manifest: &BrowserContextExportManifest,
    profile_archive: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let manifest_json = serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?;
    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let file_options =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file(BROWSER_CONTEXT_EXPORT_MANIFEST_PATH, file_options)
        .map_err(|error| error.to_string())?;
    zip.write_all(&manifest_json)
        .map_err(|error| error.to_string())?;
    if let Some(profile_archive) = profile_archive {
        zip.start_file(BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH, file_options)
            .map_err(|error| error.to_string())?;
        zip.write_all(profile_archive)
            .map_err(|error| error.to_string())?;
    }

    let cursor = zip.finish().map_err(|error| error.to_string())?;
    Ok(cursor.into_inner())
}

fn parse_browser_context_import_archive(
    bytes: &[u8],
) -> Result<ParsedBrowserContextImportArchive, String> {
    if bytes.is_empty() {
        return Err("browser context import archive must not be empty".to_string());
    }
    let cursor = Cursor::new(bytes.to_vec());
    let mut zip = zip::ZipArchive::new(cursor)
        .map_err(|error| format!("browser context import archive must be a valid zip: {error}"))?;
    let mut manifest_count = 0_u32;
    let mut profile_count = 0_u32;
    for index in 0..zip.len() {
        let file = zip
            .by_index(index)
            .map_err(|error| format!("failed to read browser context archive entry: {error}"))?;
        if file.is_dir() {
            return Err(format!(
                "browser context import archive contains unsupported directory entry {}",
                file.name()
            ));
        }
        match file.name() {
            BROWSER_CONTEXT_EXPORT_MANIFEST_PATH => manifest_count += 1,
            BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH => profile_count += 1,
            other => {
                return Err(format!(
                    "browser context import archive contains unsupported entry {other}"
                ));
            }
        }
    }
    if manifest_count != 1 {
        return Err(
            "browser context import archive must contain exactly one manifest.json".to_string(),
        );
    }
    if profile_count > 1 {
        return Err(
            "browser context import archive must contain at most one profile.tar.gz".to_string(),
        );
    }

    let mut manifest_file = zip
        .by_name(BROWSER_CONTEXT_EXPORT_MANIFEST_PATH)
        .map_err(|error| {
            format!("browser context import archive is missing manifest.json: {error}")
        })?;
    if manifest_file.size() > MAX_BROWSER_CONTEXT_EXPORT_MANIFEST_BYTES {
        return Err("browser context import manifest is too large".to_string());
    }
    let mut manifest_bytes = Vec::new();
    manifest_file
        .read_to_end(&mut manifest_bytes)
        .map_err(|error| format!("failed to read browser context import manifest: {error}"))?;
    drop(manifest_file);
    let manifest = serde_json::from_slice::<BrowserContextExportManifest>(&manifest_bytes)
        .map_err(|error| format!("browser context import manifest is invalid JSON: {error}"))?;
    validate_browser_context_import_manifest(&manifest, profile_count > 0)?;

    let profile_archive = if manifest.profile_archive_path.as_deref().is_some() {
        let mut profile_file =
            zip.by_name(BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH)
                .map_err(|error| {
                    format!("browser context import archive is missing profile.tar.gz: {error}")
                })?;
        if profile_file.size() == 0 {
            return Err("browser context profile archive must not be empty".to_string());
        }
        let mut profile_bytes = Vec::new();
        profile_file
            .read_to_end(&mut profile_bytes)
            .map_err(|error| format!("failed to read browser context profile archive: {error}"))?;
        if profile_bytes.is_empty() {
            return Err("browser context profile archive must not be empty".to_string());
        }
        Some(profile_bytes)
    } else {
        None
    };

    Ok(ParsedBrowserContextImportArchive {
        manifest,
        profile_archive,
    })
}

fn validate_browser_context_import_manifest(
    manifest: &BrowserContextExportManifest,
    archive_contains_profile: bool,
) -> Result<(), String> {
    if manifest.format_version != 1 {
        return Err(format!(
            "unsupported browser context export format version {}",
            manifest.format_version
        ));
    }
    if manifest.archive_type != "browser_context_export" {
        return Err(format!(
            "unsupported browser context archive type {}",
            manifest.archive_type
        ));
    }
    if manifest.source_context.persistence_mode != BrowserContextPersistenceMode::Reusable {
        return Err("browser context import source must be reusable".to_string());
    }
    if manifest.source_context.state != BrowserContextState::Ready {
        return Err("browser context import source must be ready".to_string());
    }
    match manifest.profile_archive_path.as_deref() {
        Some(BROWSER_CONTEXT_PROFILE_ARCHIVE_PATH) => {
            if !archive_contains_profile {
                return Err(
                    "browser context import manifest references profile.tar.gz but the archive is missing it"
                        .to_string(),
                );
            }
        }
        Some(path) => {
            return Err(format!(
                "unsupported browser context profile archive path {path}"
            ));
        }
        None => {
            if archive_contains_profile {
                return Err(
                    "browser context import archive contains profile.tar.gz but the manifest does not reference it"
                        .to_string(),
                );
            }
        }
    }
    Ok(())
}

fn browser_context_import_request_from_headers(
    headers: &HeaderMap,
    target_context_id: Uuid,
    manifest: &BrowserContextExportManifest,
) -> Result<PersistBrowserContextRequest, (StatusCode, Json<ErrorResponse>)> {
    let name = required_header_string(headers, BROWSER_CONTEXT_NAME_HEADER)?;
    let description = optional_header_string(headers, BROWSER_CONTEXT_DESCRIPTION_HEADER)?
        .or_else(|| manifest.source_context.description.clone());
    let labels = parse_optional_string_map_header(headers, BROWSER_CONTEXT_LABELS_HEADER)?
        .unwrap_or_else(|| manifest.source_context.labels.clone());
    let retention_sec = optional_u32_header(headers, BROWSER_CONTEXT_RETENTION_SEC_HEADER)?
        .or(manifest.source_context.retention_sec);
    let max_profile_storage_bytes =
        optional_u64_header(headers, BROWSER_CONTEXT_MAX_PROFILE_STORAGE_BYTES_HEADER)?
            .or(manifest.source_context.max_profile_storage_bytes);

    Ok(PersistBrowserContextRequest {
        id: Some(target_context_id),
        project_id: optional_uuid_header(headers, BROWSER_CONTEXT_PROJECT_ID_HEADER)?,
        name,
        description,
        labels,
        persistence_mode: BrowserContextPersistenceMode::Reusable,
        retention_sec,
        max_profile_storage_bytes,
    })
}

fn optional_header_string(
    headers: &HeaderMap,
    name: &str,
) -> Result<Option<String>, (StatusCode, Json<ErrorResponse>)> {
    let Some(raw) = headers.get(name) else {
        return Ok(None);
    };
    let value = raw
        .to_str()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("header {name} must be valid UTF-8"),
                }),
            )
        })?
        .trim()
        .to_string();
    Ok((!value.is_empty()).then_some(value))
}

fn optional_u32_header(
    headers: &HeaderMap,
    name: &str,
) -> Result<Option<u32>, (StatusCode, Json<ErrorResponse>)> {
    optional_header_string(headers, name)?
        .map(|value| {
            value.parse::<u32>().map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("header {name} must be a positive integer: {error}"),
                    }),
                )
            })
        })
        .transpose()
}

fn optional_u64_header(
    headers: &HeaderMap,
    name: &str,
) -> Result<Option<u64>, (StatusCode, Json<ErrorResponse>)> {
    optional_header_string(headers, name)?
        .map(|value| {
            value.parse::<u64>().map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("header {name} must be a positive integer: {error}"),
                    }),
                )
            })
        })
        .transpose()
}

fn optional_uuid_header(
    headers: &HeaderMap,
    name: &str,
) -> Result<Option<Uuid>, (StatusCode, Json<ErrorResponse>)> {
    optional_header_string(headers, name)?
        .map(|value| {
            value.parse::<Uuid>().map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("header {name} must be a valid UUID: {error}"),
                    }),
                )
            })
        })
        .transpose()
}

fn parse_optional_string_map_header(
    headers: &HeaderMap,
    name: &str,
) -> Result<Option<HashMap<String, String>>, (StatusCode, Json<ErrorResponse>)> {
    let Some(value) = parse_optional_json_object_header(headers, name)? else {
        return Ok(None);
    };
    let object = value.as_object().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("header {name} must contain a JSON object"),
            }),
        )
    })?;
    let mut map = HashMap::new();
    for (key, value) in object {
        let Some(value) = value.as_str() else {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("header {name} values must be strings"),
                }),
            ));
        };
        map.insert(key.clone(), value.to_string());
    }
    Ok(Some(map))
}

fn bad_request(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse { error: message }),
    )
}

fn map_browser_context_import_runtime_error(
    error: SessionManagerError,
) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        SessionManagerError::InvalidConfiguration(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        _ => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

async fn browser_context_usage_by_id(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    context_ids: &[Uuid],
) -> Result<HashMap<Uuid, BrowserContextUsageResource>, (StatusCode, Json<ErrorResponse>)> {
    if context_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let requested_context_ids = context_ids.iter().copied().collect::<HashSet<_>>();
    let mut usage_by_context = HashMap::new();
    let storage_by_context = state
        .session_manager
        .browser_context_profile_storage_bytes(context_ids)
        .await
        .unwrap_or_else(|error| {
            warn!(
                error = %error,
                "could not inspect browser context profile storage usage",
            );
            HashMap::new()
        });
    let sessions = state
        .session_store
        .list_sessions_for_owner(principal)
        .await
        .map_err(map_session_store_error)?;

    for session in sessions {
        let Some(context_id) = reusable_context_id(&session) else {
            continue;
        };
        if !requested_context_ids.contains(&context_id) {
            continue;
        }
        usage_by_context
            .entry(context_id)
            .or_insert_with(BrowserContextUsageResource::default)
            .visible_session_count += 1;
    }

    for context_id in requested_context_ids {
        let Some(active_session_id) = state
            .session_manager
            .active_browser_context_session_id(context_id)
            .await
        else {
            continue;
        };
        let usage = usage_by_context
            .entry(context_id)
            .or_insert_with(BrowserContextUsageResource::default);
        usage.active_runtime_session_count = 1;
        usage.active_runtime_session_id = Some(active_session_id);
    }

    for (context_id, storage_bytes) in storage_by_context {
        let usage = usage_by_context
            .entry(context_id)
            .or_insert_with(BrowserContextUsageResource::default);
        usage.profile_storage_bytes = Some(storage_bytes);
    }

    Ok(usage_by_context)
}

fn reusable_context_id(session: &StoredSession) -> Option<Uuid> {
    (session.browser_context.mode == SessionBrowserContextMode::Reusable)
        .then_some(session.browser_context.context_id)
        .flatten()
}
