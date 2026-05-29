use std::collections::HashMap;

use axum::routing::get;
use chrono::DateTime;
use serde::Serialize;

use super::*;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum IdentityPrincipalType {
    User,
    ServicePrincipal,
    LegacyDevToken,
}

#[derive(Debug, Clone, Serialize)]
struct IdentityPrincipalResource {
    subject: String,
    issuer: String,
    display_name: Option<String>,
    client_id: Option<String>,
    principal_type: IdentityPrincipalType,
}

#[derive(Debug, Clone, Serialize)]
struct IdentityResourceCounts {
    projects: usize,
    service_principals: usize,
    sessions: usize,
    active_sessions: usize,
    session_templates: usize,
    browser_contexts: usize,
    egress_profiles: usize,
    credential_bindings: usize,
    file_workspaces: usize,
    workflow_definitions: usize,
    workflow_runs: usize,
    active_workflow_runs: usize,
    automation_tasks: usize,
    active_automation_tasks: usize,
    extension_definitions: usize,
    delegated_principals: usize,
}

#[derive(Debug, Clone, Serialize)]
struct IdentityDelegatedPrincipalResource {
    client_id: String,
    issuer: String,
    display_name: Option<String>,
    registered: bool,
    registered_service_principal_id: Option<Uuid>,
    state: Option<ServicePrincipalState>,
    session_count: usize,
    active_session_count: usize,
    session_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
struct IdentityServicePrincipalReviewResource {
    #[serde(flatten)]
    service_principal: ServicePrincipalResource,
    delegated_session_count: usize,
    active_delegated_session_count: usize,
    delegated_session_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
struct IdentityAccessReviewResponse {
    principal: IdentityPrincipalResource,
    generated_at: DateTime<Utc>,
    projects: Vec<ProjectResource>,
    resource_counts: IdentityResourceCounts,
    service_principals: Vec<IdentityServicePrincipalReviewResource>,
    delegated_principals: Vec<IdentityDelegatedPrincipalResource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DelegatedPrincipalKey {
    client_id: String,
    issuer: String,
    display_name: Option<String>,
}

pub(super) fn identity_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/api/v1/identity/me", get(get_current_identity))
        .route(
            "/api/v1/identity/access-review",
            get(get_identity_access_review),
        )
}

async fn get_current_identity(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<IdentityPrincipalResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    mark_current_service_principal_seen(&state, &principal).await?;
    Ok(Json(identity_principal_resource(&principal)))
}

async fn get_identity_access_review(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<IdentityAccessReviewResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    mark_current_service_principal_seen(&state, &principal).await?;
    let projects = state
        .session_store
        .list_projects_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let service_principals = state
        .session_store
        .list_service_principals_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let sessions = state
        .session_store
        .list_sessions_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let session_templates = state
        .session_store
        .list_session_templates_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let browser_contexts = state
        .session_store
        .list_browser_contexts_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let egress_profiles = state
        .session_store
        .list_egress_profiles_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let credential_bindings = state
        .session_store
        .list_credential_bindings_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let file_workspaces = state
        .session_store
        .list_file_workspaces_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let workflow_definitions = state
        .session_store
        .list_workflow_definitions_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let workflow_runs = state
        .session_store
        .list_workflow_runs_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let automation_tasks = state
        .session_store
        .list_automation_tasks_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let extension_definitions = state
        .session_store
        .list_extension_definitions_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;

    let now = Utc::now();
    let projects = project_resources(&state, &principal, projects).await?;
    let active_sessions = sessions
        .iter()
        .filter(|session| session.state.is_runtime_candidate())
        .count();
    let delegated_principals = delegated_principal_resources(&sessions, &service_principals);
    let service_principal_reviews =
        service_principal_review_resources(&service_principals, &sessions);
    let resource_counts = IdentityResourceCounts {
        projects: projects.len(),
        service_principals: service_principals.len(),
        sessions: sessions.len(),
        active_sessions,
        session_templates: session_templates.len(),
        browser_contexts: browser_contexts.len(),
        egress_profiles: egress_profiles.len(),
        credential_bindings: credential_bindings.len(),
        file_workspaces: file_workspaces.len(),
        workflow_definitions: workflow_definitions.len(),
        workflow_runs: workflow_runs.len(),
        active_workflow_runs: workflow_runs
            .iter()
            .filter(|run| !run.state.is_terminal())
            .count(),
        automation_tasks: automation_tasks.len(),
        active_automation_tasks: automation_tasks
            .iter()
            .filter(|task| !task.state.is_terminal())
            .count(),
        extension_definitions: extension_definitions.len(),
        delegated_principals: delegated_principals.len(),
    };

    Ok(Json(IdentityAccessReviewResponse {
        principal: identity_principal_resource(&principal),
        generated_at: now,
        projects,
        resource_counts,
        service_principals: service_principal_reviews,
        delegated_principals,
    }))
}

async fn mark_current_service_principal_seen(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let Some(client_id) = principal.client_id.as_deref() else {
        return Ok(());
    };
    state
        .session_store
        .mark_service_principal_seen_for_owner(principal, &principal.issuer, client_id)
        .await
        .map_err(map_session_store_error)?;
    Ok(())
}

async fn project_resources(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    projects: Vec<StoredProject>,
) -> Result<Vec<ProjectResource>, (StatusCode, Json<ErrorResponse>)> {
    let mut resources = Vec::with_capacity(projects.len());
    for project in projects {
        let active_sessions = state
            .session_store
            .count_active_sessions_for_project(principal, project.id)
            .await
            .map_err(map_session_store_error)?;
        resources.push(project.to_resource(active_sessions, Utc::now()));
    }
    Ok(resources)
}

fn identity_principal_resource(principal: &AuthenticatedPrincipal) -> IdentityPrincipalResource {
    IdentityPrincipalResource {
        subject: principal.subject.clone(),
        issuer: principal.issuer.clone(),
        display_name: principal.display_name.clone(),
        client_id: principal.client_id.clone(),
        principal_type: classify_principal(principal),
    }
}

fn classify_principal(principal: &AuthenticatedPrincipal) -> IdentityPrincipalType {
    if principal.issuer == "bpane-gateway" && principal.subject.starts_with("legacy-dev-token:") {
        return IdentityPrincipalType::LegacyDevToken;
    }

    let service_account_display = principal
        .display_name
        .as_deref()
        .is_some_and(|value| value.starts_with("service-account-"));
    let client_credentials_display = principal
        .client_id
        .as_deref()
        .is_some_and(|client_id| principal.display_name.as_deref() == Some(client_id));
    if service_account_display || client_credentials_display {
        IdentityPrincipalType::ServicePrincipal
    } else {
        IdentityPrincipalType::User
    }
}

fn delegated_principal_resources(
    sessions: &[StoredSession],
    service_principals: &[StoredServicePrincipal],
) -> Vec<IdentityDelegatedPrincipalResource> {
    let service_principal_index = service_principals
        .iter()
        .map(|service_principal| {
            (
                DelegatedPrincipalKey {
                    client_id: service_principal.client_id.clone(),
                    issuer: service_principal.issuer.clone(),
                    display_name: None,
                },
                service_principal,
            )
        })
        .collect::<HashMap<_, _>>();
    let mut groups: HashMap<DelegatedPrincipalKey, IdentityDelegatedPrincipalResource> =
        HashMap::new();
    for session in sessions {
        let Some(delegate) = &session.automation_delegate else {
            continue;
        };
        let key = DelegatedPrincipalKey {
            client_id: delegate.client_id.clone(),
            issuer: delegate.issuer.clone(),
            display_name: delegate.display_name.clone(),
        };
        let entry = groups
            .entry(key)
            .or_insert_with(|| IdentityDelegatedPrincipalResource {
                client_id: delegate.client_id.clone(),
                issuer: delegate.issuer.clone(),
                display_name: delegate.display_name.clone(),
                registered: false,
                registered_service_principal_id: None,
                state: None,
                session_count: 0,
                active_session_count: 0,
                session_ids: Vec::new(),
            });
        entry.session_count += 1;
        if session.state.is_runtime_candidate() {
            entry.active_session_count += 1;
        }
        entry.session_ids.push(session.id);
    }

    let mut resources = groups.into_values().collect::<Vec<_>>();
    for resource in &mut resources {
        if let Some(service_principal) = service_principal_index.get(&DelegatedPrincipalKey {
            client_id: resource.client_id.clone(),
            issuer: resource.issuer.clone(),
            display_name: None,
        }) {
            resource.registered = true;
            resource.registered_service_principal_id = Some(service_principal.id);
            resource.state = Some(service_principal.state);
        }
        resource.session_ids.sort_unstable();
    }
    resources.sort_by(|left, right| {
        left.client_id
            .cmp(&right.client_id)
            .then(left.issuer.cmp(&right.issuer))
    });
    resources
}

fn service_principal_review_resources(
    service_principals: &[StoredServicePrincipal],
    sessions: &[StoredSession],
) -> Vec<IdentityServicePrincipalReviewResource> {
    let mut resources = service_principals
        .iter()
        .map(|service_principal| {
            let mut delegated_session_ids = sessions
                .iter()
                .filter(|session| {
                    session
                        .automation_delegate
                        .as_ref()
                        .is_some_and(|delegate| {
                            delegate.client_id == service_principal.client_id
                                && delegate.issuer == service_principal.issuer
                        })
                })
                .map(|session| session.id)
                .collect::<Vec<_>>();
            delegated_session_ids.sort_unstable();
            let active_delegated_session_count = sessions
                .iter()
                .filter(|session| {
                    session.state.is_runtime_candidate()
                        && session
                            .automation_delegate
                            .as_ref()
                            .is_some_and(|delegate| {
                                delegate.client_id == service_principal.client_id
                                    && delegate.issuer == service_principal.issuer
                            })
                })
                .count();
            IdentityServicePrincipalReviewResource {
                service_principal: service_principal.to_resource(),
                delegated_session_count: delegated_session_ids.len(),
                active_delegated_session_count,
                delegated_session_ids,
            }
        })
        .collect::<Vec<_>>();
    resources.sort_by(|left, right| {
        left.service_principal
            .client_id
            .cmp(&right.service_principal.client_id)
            .then(
                left.service_principal
                    .issuer
                    .cmp(&right.service_principal.issuer),
            )
    });
    resources
}
