use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio_postgres::{Client, Connection, NoTls, Row, Socket};
use uuid::Uuid;

use crate::auth::AuthenticatedPrincipal;
use crate::runtime_manager::RuntimeProfile;

const DEFAULT_VIEWPORT_WIDTH: u16 = 1600;
const DEFAULT_VIEWPORT_HEIGHT: u16 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLifecycleState {
    Pending,
    Starting,
    Ready,
    Active,
    Idle,
    Stopping,
    Stopped,
    Failed,
    Expired,
}

impl SessionLifecycleState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Active => "active",
            Self::Idle => "idle",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }

    pub fn is_runtime_candidate(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::Starting | Self::Ready | Self::Active | Self::Idle
        )
    }
}

impl FromStr for SessionLifecycleState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "starting" => Ok(Self::Starting),
            "ready" => Ok(Self::Ready),
            "active" => Ok(Self::Active),
            "idle" => Ok(Self::Idle),
            "stopping" => Ok(Self::Stopping),
            "stopped" => Ok(Self::Stopped),
            "failed" => Ok(Self::Failed),
            "expired" => Ok(Self::Expired),
            _ => Err("unknown session lifecycle state"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOwnerMode {
    Collaborative,
    ExclusiveBrowserOwner,
}

impl SessionOwnerMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Collaborative => "collaborative",
            Self::ExclusiveBrowserOwner => "exclusive_browser_owner",
        }
    }
}

impl FromStr for SessionOwnerMode {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "collaborative" => Ok(Self::Collaborative),
            "exclusive_browser_owner" => Ok(Self::ExclusiveBrowserOwner),
            _ => Err("unknown session owner mode"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionViewport {
    pub width: u16,
    pub height: u16,
}

impl Default for SessionViewport {
    fn default() -> Self {
        Self {
            width: DEFAULT_VIEWPORT_WIDTH,
            height: DEFAULT_VIEWPORT_HEIGHT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionOwner {
    pub subject: String,
    pub issuer: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAutomationDelegate {
    pub client_id: String,
    pub issuer: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCapabilities {
    pub browser_input: bool,
    pub clipboard: bool,
    pub audio: bool,
    pub microphone: bool,
    pub camera: bool,
    pub file_transfer: bool,
    pub resize: bool,
}

impl Default for SessionCapabilities {
    fn default() -> Self {
        Self {
            browser_input: true,
            clipboard: true,
            audio: true,
            microphone: true,
            camera: true,
            file_transfer: true,
            resize: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionConnectInfo {
    pub gateway_url: String,
    pub transport_path: String,
    pub auth_type: String,
    pub ticket_path: Option<String>,
    pub compatibility_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionResource {
    pub id: Uuid,
    pub state: SessionLifecycleState,
    pub template_id: Option<String>,
    pub owner_mode: SessionOwnerMode,
    pub viewport: SessionViewport,
    pub capabilities: SessionCapabilities,
    pub owner: SessionOwner,
    pub automation_delegate: Option<SessionAutomationDelegate>,
    pub idle_timeout_sec: Option<u32>,
    pub labels: HashMap<String, String>,
    pub integration_context: Option<Value>,
    pub connect: SessionConnectInfo,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub stopped_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub template_id: Option<String>,
    #[serde(default)]
    pub owner_mode: Option<SessionOwnerMode>,
    #[serde(default)]
    pub viewport: Option<SessionViewport>,
    #[serde(default)]
    pub idle_timeout_sec: Option<u32>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub integration_context: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SetAutomationDelegateRequest {
    pub client_id: String,
    #[serde(default)]
    pub issuer: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionResource>,
}

#[derive(Debug, Clone)]
pub struct StoredSession {
    pub id: Uuid,
    pub state: SessionLifecycleState,
    pub template_id: Option<String>,
    pub owner_mode: SessionOwnerMode,
    pub viewport: SessionViewport,
    pub owner: SessionOwner,
    pub automation_delegate: Option<SessionAutomationDelegate>,
    pub idle_timeout_sec: Option<u32>,
    pub labels: HashMap<String, String>,
    pub integration_context: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub stopped_at: Option<DateTime<Utc>>,
}

impl StoredSession {
    pub fn to_resource(
        &self,
        public_gateway_url: &str,
        compatibility_mode: &str,
        state_override: Option<SessionLifecycleState>,
    ) -> SessionResource {
        SessionResource {
            id: self.id,
            state: state_override.unwrap_or(self.state),
            template_id: self.template_id.clone(),
            owner_mode: self.owner_mode,
            viewport: self.viewport.clone(),
            capabilities: SessionCapabilities::default(),
            owner: self.owner.clone(),
            automation_delegate: self.automation_delegate.clone(),
            idle_timeout_sec: self.idle_timeout_sec,
            labels: self.labels.clone(),
            integration_context: self.integration_context.clone(),
            connect: SessionConnectInfo {
                gateway_url: public_gateway_url.to_string(),
                transport_path: "/session".to_string(),
                auth_type: "session_connect_ticket".to_string(),
                ticket_path: Some(format!("/api/v1/sessions/{}/access-tokens", self.id)),
                compatibility_mode: compatibility_mode.to_string(),
            },
            created_at: self.created_at,
            updated_at: self.updated_at,
            stopped_at: self.stopped_at,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SessionStoreError {
    ActiveSessionConflict { max_runtime_sessions: usize },
    InvalidRequest(String),
    Backend(String),
}

impl std::fmt::Display for SessionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ActiveSessionConflict {
                max_runtime_sessions,
            } => {
                write!(
                    f,
                    "the current gateway runtime only supports {} active runtime-backed session{}",
                    max_runtime_sessions,
                    if *max_runtime_sessions == 1 { "" } else { "s" }
                )
            }
            Self::InvalidRequest(message) => write!(f, "{message}"),
            Self::Backend(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for SessionStoreError {}

#[derive(Clone)]
pub struct SessionStore {
    backend: SessionStoreBackend,
}

#[derive(Debug, Clone)]
struct SessionStoreConfig {
    runtime_binding: String,
    compatibility_mode: String,
    max_runtime_candidates: usize,
}

#[derive(Clone)]
enum SessionStoreBackend {
    InMemory(Arc<InMemorySessionStore>),
    Postgres(Arc<PostgresSessionStore>),
}

impl From<RuntimeProfile> for SessionStoreConfig {
    fn from(runtime_profile: RuntimeProfile) -> Self {
        Self {
            runtime_binding: runtime_profile.runtime_binding,
            compatibility_mode: runtime_profile.compatibility_mode,
            max_runtime_candidates: runtime_profile.max_runtime_sessions,
        }
    }
}

#[cfg(test)]
fn legacy_runtime_profile() -> RuntimeProfile {
    RuntimeProfile {
        runtime_binding: "legacy_single_session".to_string(),
        compatibility_mode: "legacy_single_runtime".to_string(),
        max_runtime_sessions: 1,
        supports_legacy_global_routes: true,
    }
}

impl SessionStore {
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::in_memory_with_config(legacy_runtime_profile())
    }

    pub fn in_memory_with_config(runtime_profile: RuntimeProfile) -> Self {
        Self {
            backend: SessionStoreBackend::InMemory(Arc::new(InMemorySessionStore {
                sessions: Mutex::new(Vec::new()),
                config: SessionStoreConfig::from(runtime_profile),
            })),
        }
    }

    pub async fn from_database_url_with_config(
        database_url: &str,
        runtime_profile: RuntimeProfile,
    ) -> Result<Self, SessionStoreError> {
        let (client, connection) = connect_to_postgres_with_retry(database_url).await?;
        tokio::spawn(async move {
            if let Err(error) = connection.await {
                tracing::error!("postgres connection error: {error}");
            }
        });
        let store = PostgresSessionStore {
            client: Mutex::new(client),
            config: SessionStoreConfig::from(runtime_profile),
        };
        store.migrate().await?;
        Ok(Self {
            backend: SessionStoreBackend::Postgres(Arc::new(store)),
        })
    }

    pub fn compatibility_mode(&self) -> &str {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => &store.config.compatibility_mode,
            SessionStoreBackend::Postgres(store) => &store.config.compatibility_mode,
        }
    }

    pub async fn create_session(
        &self,
        principal: &AuthenticatedPrincipal,
        request: CreateSessionRequest,
        owner_mode: SessionOwnerMode,
    ) -> Result<StoredSession, SessionStoreError> {
        validate_create_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_session(principal, request, owner_mode).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_session(principal, request, owner_mode).await
            }
        }
    }

    pub async fn list_sessions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_sessions_for_owner(principal).await,
            SessionStoreBackend::Postgres(store) => store.list_sessions_for_owner(principal).await,
        }
    }

    pub async fn get_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_session_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_session_for_owner(principal, id).await
            }
        }
    }

    pub async fn stop_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.stop_session_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.stop_session_for_owner(principal, id).await
            }
        }
    }

    pub async fn get_session_for_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_session_for_principal(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_session_for_principal(principal, id).await
            }
        }
    }

    pub async fn get_session_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.get_session_by_id(id).await,
            SessionStoreBackend::Postgres(store) => store.get_session_by_id(id).await,
        }
    }

    pub async fn get_runtime_candidate_session(
        &self,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.get_runtime_candidate_session().await,
            SessionStoreBackend::Postgres(store) => store.get_runtime_candidate_session().await,
        }
    }

    pub async fn set_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: SetAutomationDelegateRequest,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        validate_automation_delegate_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .set_automation_delegate_for_owner(principal, id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .set_automation_delegate_for_owner(principal, id, request)
                    .await
            }
        }
    }

    pub async fn clear_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .clear_automation_delegate_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .clear_automation_delegate_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn mark_session_active(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Active)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Active)
                    .await
            }
        }
    }

    pub async fn mark_session_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Idle)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Idle)
                    .await
            }
        }
    }

    pub async fn stop_session_if_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.stop_session_if_idle(id).await,
            SessionStoreBackend::Postgres(store) => store.stop_session_if_idle(id).await,
        }
    }
}

async fn connect_to_postgres_with_retry(
    database_url: &str,
) -> Result<(Client, Connection<Socket, tokio_postgres::tls::NoTlsStream>), SessionStoreError> {
    let max_attempts = 30;
    let mut last_error = String::new();
    for attempt in 0..max_attempts {
        match tokio_postgres::connect(database_url, NoTls).await {
            Ok(connection) => return Ok(connection),
            Err(error) => {
                last_error = error.to_string();
                if attempt + 1 < max_attempts {
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    Err(SessionStoreError::Backend(format!(
        "failed to connect to postgres after retries: {last_error}"
    )))
}

fn validate_create_request(request: &CreateSessionRequest) -> Result<(), SessionStoreError> {
    if let Some(viewport) = &request.viewport {
        if viewport.width == 0 || viewport.height == 0 {
            return Err(SessionStoreError::InvalidRequest(
                "viewport width and height must be greater than zero".to_string(),
            ));
        }
    }
    if let Some(idle_timeout_sec) = request.idle_timeout_sec {
        if idle_timeout_sec == 0 {
            return Err(SessionStoreError::InvalidRequest(
                "idle_timeout_sec must be greater than zero when provided".to_string(),
            ));
        }
    }
    if let Some(integration_context) = &request.integration_context {
        if !integration_context.is_object() {
            return Err(SessionStoreError::InvalidRequest(
                "integration_context must be a JSON object when provided".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_automation_delegate_request(
    request: &SetAutomationDelegateRequest,
) -> Result<(), SessionStoreError> {
    if request.client_id.trim().is_empty() {
        return Err(SessionStoreError::InvalidRequest(
            "client_id must not be empty".to_string(),
        ));
    }
    if let Some(issuer) = &request.issuer {
        if issuer.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "issuer must not be empty when provided".to_string(),
            ));
        }
    }
    Ok(())
}

struct InMemorySessionStore {
    sessions: Mutex<Vec<StoredSession>>,
    config: SessionStoreConfig,
}

impl InMemorySessionStore {
    async fn get_session_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .find(|session| session.id == id)
            .cloned();
        Ok(session)
    }

    async fn create_session(
        &self,
        principal: &AuthenticatedPrincipal,
        request: CreateSessionRequest,
        owner_mode: SessionOwnerMode,
    ) -> Result<StoredSession, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let active_runtime_candidates = sessions
            .iter()
            .filter(|session| session.state.is_runtime_candidate())
            .count();
        if active_runtime_candidates >= self.config.max_runtime_candidates {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }

        let now = Utc::now();
        let session = StoredSession {
            id: Uuid::now_v7(),
            state: SessionLifecycleState::Ready,
            template_id: request.template_id,
            owner_mode,
            viewport: request.viewport.unwrap_or_default(),
            owner: SessionOwner {
                subject: principal.subject.clone(),
                issuer: principal.issuer.clone(),
                display_name: principal.display_name.clone(),
            },
            automation_delegate: None,
            idle_timeout_sec: request.idle_timeout_sec,
            labels: request.labels,
            integration_context: request.integration_context,
            created_at: now,
            updated_at: now,
            stopped_at: None,
        };
        sessions.push(session.clone());
        Ok(session)
    }

    async fn list_sessions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSession>, SessionStoreError> {
        let mut sessions = self
            .sessions
            .lock()
            .await
            .iter()
            .filter(|session| {
                session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(sessions)
    }

    async fn get_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .find(|session| {
                session.id == id
                    && session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
            })
            .cloned();
        Ok(session)
    }

    async fn get_session_for_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .find(|session| session.id == id && session_visible_to_principal(session, principal))
            .cloned();
        Ok(session)
    }

    async fn get_runtime_candidate_session(
        &self,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .filter(|session| session.state.is_runtime_candidate())
            .max_by(|left, right| left.updated_at.cmp(&right.updated_at))
            .cloned();
        Ok(session)
    }

    async fn stop_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| {
            session.id == id
                && session.owner.subject == principal.subject
                && session.owner.issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        if session.state != SessionLifecycleState::Stopped {
            session.state = SessionLifecycleState::Stopped;
            session.updated_at = Utc::now();
            session.stopped_at = Some(session.updated_at);
        }

        Ok(Some(session.clone()))
    }

    async fn mark_session_state(
        &self,
        id: Uuid,
        state: SessionLifecycleState,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| session.id == id) else {
            return Ok(None);
        };

        if !session.state.is_runtime_candidate() {
            return Ok(Some(session.clone()));
        }

        session.state = state;
        session.updated_at = Utc::now();
        Ok(Some(session.clone()))
    }

    async fn stop_session_if_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| session.id == id) else {
            return Ok(None);
        };

        if !matches!(
            session.state,
            SessionLifecycleState::Ready | SessionLifecycleState::Idle
        ) {
            return Ok(Some(session.clone()));
        }

        session.state = SessionLifecycleState::Stopped;
        session.updated_at = Utc::now();
        session.stopped_at = Some(session.updated_at);
        Ok(Some(session.clone()))
    }

    async fn set_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: SetAutomationDelegateRequest,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| {
            session.id == id
                && session.owner.subject == principal.subject
                && session.owner.issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        session.automation_delegate = Some(SessionAutomationDelegate {
            client_id: request.client_id,
            issuer: request.issuer.unwrap_or_else(|| principal.issuer.clone()),
            display_name: request.display_name,
        });
        session.updated_at = Utc::now();

        Ok(Some(session.clone()))
    }

    async fn clear_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| {
            session.id == id
                && session.owner.subject == principal.subject
                && session.owner.issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        session.automation_delegate = None;
        session.updated_at = Utc::now();

        Ok(Some(session.clone()))
    }
}

struct PostgresSessionStore {
    client: Mutex<Client>,
    config: SessionStoreConfig,
}

impl PostgresSessionStore {
    async fn migrate(&self) -> Result<(), SessionStoreError> {
        self.client
            .lock()
            .await
            .batch_execute(
                r#"
                CREATE TABLE IF NOT EXISTS control_sessions (
                    id UUID PRIMARY KEY,
                    owner_subject TEXT NOT NULL,
                    owner_issuer TEXT NOT NULL,
                    owner_display_name TEXT NULL,
                    automation_owner_client_id TEXT NULL,
                    automation_owner_issuer TEXT NULL,
                    automation_owner_display_name TEXT NULL,
                    state TEXT NOT NULL,
                    template_id TEXT NULL,
                    owner_mode TEXT NOT NULL,
                    viewport_width INTEGER NOT NULL CHECK (viewport_width > 0 AND viewport_width <= 65535),
                    viewport_height INTEGER NOT NULL CHECK (viewport_height > 0 AND viewport_height <= 65535),
                    idle_timeout_sec INTEGER NULL CHECK (idle_timeout_sec IS NULL OR idle_timeout_sec > 0),
                    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                    integration_context JSONB NULL,
                    runtime_binding TEXT NOT NULL DEFAULT 'legacy_single_session',
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    stopped_at TIMESTAMPTZ NULL
                );

                CREATE INDEX IF NOT EXISTS idx_control_sessions_owner_created
                    ON control_sessions (owner_subject, owner_issuer, created_at DESC);

                CREATE INDEX IF NOT EXISTS idx_control_sessions_runtime_state
                    ON control_sessions (runtime_binding, state, created_at DESC);

                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS automation_owner_client_id TEXT NULL;
                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS automation_owner_issuer TEXT NULL;
                ALTER TABLE control_sessions
                    ADD COLUMN IF NOT EXISTS automation_owner_display_name TEXT NULL;
                "#,
            )
            .await
            .map_err(|error| SessionStoreError::Backend(format!("failed to migrate postgres schema: {error}")))
    }

    async fn create_session(
        &self,
        principal: &AuthenticatedPrincipal,
        request: CreateSessionRequest,
        owner_mode: SessionOwnerMode,
    ) -> Result<StoredSession, SessionStoreError> {
        let mut client = self.client.lock().await;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let existing = transaction
            .query_opt(
                r#"
                SELECT COUNT(*)::BIGINT AS session_count
                FROM control_sessions
                WHERE runtime_binding = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                "#,
                &[&self.config.runtime_binding],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to check active sessions: {error}"))
            })?;
        let active_runtime_candidates = existing
            .as_ref()
            .map(|row| row.get::<_, i64>("session_count"))
            .unwrap_or(0);
        if active_runtime_candidates >= self.config.max_runtime_candidates as i64 {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }

        let viewport = request.viewport.unwrap_or_default();
        let now = Utc::now();
        let labels_value = json_labels(&request.labels);
        let session_id = Uuid::now_v7();
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_sessions (
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    runtime_binding,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::jsonb, $12::jsonb, $13, $14, $14)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[
                    &session_id,
                    &principal.subject,
                    &principal.issuer,
                    &principal.display_name,
                    &SessionLifecycleState::Ready.as_str(),
                    &request.template_id,
                    &owner_mode.as_str(),
                    &(viewport.width as i32),
                    &(viewport.height as i32),
                    &request.idle_timeout_sec.map(|value| value as i32),
                    &labels_value,
                    &request.integration_context,
                    &self.config.runtime_binding,
                    &now,
                ],
            )
            .await
            .map_err(|error| SessionStoreError::Backend(format!("failed to insert session: {error}")))?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_session(&row)
    }

    async fn list_sessions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSession>, SessionStoreError> {
        let rows = self
            .client
            .lock()
            .await
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    created_at,
                    updated_at,
                    stopped_at
                FROM control_sessions
                WHERE owner_subject = $1 AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list sessions: {error}"))
            })?;

        rows.iter().map(row_to_stored_session).collect()
    }

    async fn get_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    created_at,
                    updated_at,
                    stopped_at
                FROM control_sessions
                WHERE id = $1 AND owner_subject = $2 AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn get_session_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    created_at,
                    updated_at,
                    stopped_at
                FROM control_sessions
                WHERE id = $1
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session by id: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn get_session_for_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    created_at,
                    updated_at,
                    stopped_at
                FROM control_sessions
                WHERE id = $1
                  AND (
                    (owner_subject = $2 AND owner_issuer = $3)
                    OR (
                        automation_owner_client_id IS NOT NULL
                        AND automation_owner_issuer = $3
                        AND automation_owner_client_id = $4
                    )
                  )
                "#,
                &[
                    &id,
                    &principal.subject,
                    &principal.issuer,
                    &principal.client_id,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session for principal: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn get_runtime_candidate_session(
        &self,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    created_at,
                    updated_at,
                    stopped_at
                FROM control_sessions
                WHERE runtime_binding = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                ORDER BY updated_at DESC
                LIMIT 1
                "#,
                &[&self.config.runtime_binding],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load runtime candidate session: {error}"
                ))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn stop_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    state = 'stopped',
                    updated_at = NOW(),
                    stopped_at = COALESCE(stopped_at, NOW())
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to stop session: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn mark_session_state(
        &self,
        id: Uuid,
        state: SessionLifecycleState,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    state = $2,
                    updated_at = NOW()
                WHERE id = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[&id, &state.as_str()],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to update session state: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn stop_session_if_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    state = 'stopped',
                    updated_at = NOW(),
                    stopped_at = COALESCE(stopped_at, NOW())
                WHERE id = $1
                  AND state IN ('ready', 'idle')
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to stop idle session: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn set_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: SetAutomationDelegateRequest,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let issuer = request.issuer.unwrap_or_else(|| principal.issuer.clone());
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    automation_owner_client_id = $4,
                    automation_owner_issuer = $5,
                    automation_owner_display_name = $6,
                    updated_at = NOW()
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[
                    &id,
                    &principal.subject,
                    &principal.issuer,
                    &request.client_id,
                    &issuer,
                    &request.display_name,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to set automation delegate: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    async fn clear_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .client
            .lock()
            .await
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    automation_owner_client_id = NULL,
                    automation_owner_issuer = NULL,
                    automation_owner_display_name = NULL,
                    updated_at = NOW()
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    automation_owner_client_id,
                    automation_owner_issuer,
                    automation_owner_display_name,
                    state,
                    template_id,
                    owner_mode,
                    viewport_width,
                    viewport_height,
                    idle_timeout_sec,
                    labels,
                    integration_context,
                    created_at,
                    updated_at,
                    stopped_at
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to clear automation delegate: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }
}

fn json_labels(labels: &HashMap<String, String>) -> Value {
    let mut object = JsonMap::new();
    for (key, value) in labels {
        object.insert(key.clone(), Value::String(value.clone()));
    }
    Value::Object(object)
}

fn session_visible_to_principal(
    session: &StoredSession,
    principal: &AuthenticatedPrincipal,
) -> bool {
    if session.owner.subject == principal.subject && session.owner.issuer == principal.issuer {
        return true;
    }

    let Some(delegate) = &session.automation_delegate else {
        return false;
    };

    principal.client_id.as_deref() == Some(delegate.client_id.as_str())
        && principal.issuer == delegate.issuer
}

fn row_to_stored_session(row: &Row) -> Result<StoredSession, SessionStoreError> {
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
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        stopped_at: row.get("stopped_at"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(subject: &str) -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: subject.to_string(),
            issuer: "https://issuer.example".to_string(),
            display_name: Some(subject.to_string()),
            client_id: None,
        }
    }

    fn service_principal(subject: &str, client_id: &str) -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: subject.to_string(),
            issuer: "https://issuer.example".to_string(),
            display_name: Some(client_id.to_string()),
            client_id: Some(client_id.to_string()),
        }
    }

    #[tokio::test]
    async fn in_memory_store_scopes_sessions_to_owner() {
        let store = SessionStore::in_memory();
        let alpha = principal("alpha");
        let bravo = principal("bravo");

        let created = store
            .create_session(
                &alpha,
                CreateSessionRequest {
                    template_id: Some("default".to_string()),
                    owner_mode: None,
                    viewport: Some(SessionViewport {
                        width: 1920,
                        height: 1080,
                    }),
                    idle_timeout_sec: Some(600),
                    labels: HashMap::from([("suite".to_string(), "smoke".to_string())]),
                    integration_context: Some(serde_json::json!({ "ticket": "BPANE-6" })),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let alpha_sessions = store.list_sessions_for_owner(&alpha).await.unwrap();
        assert_eq!(alpha_sessions.len(), 1);
        assert_eq!(alpha_sessions[0].id, created.id);

        let bravo_sessions = store.list_sessions_for_owner(&bravo).await.unwrap();
        assert!(bravo_sessions.is_empty());
        assert!(store
            .get_session_for_owner(&bravo, created.id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn in_memory_store_limits_legacy_runtime_to_one_active_session() {
        let store = SessionStore::in_memory();
        let alpha = principal("alpha");

        store
            .create_session(
                &alpha,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let error = store
            .create_session(
                &alpha,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: 1
            }
        ));
    }

    #[tokio::test]
    async fn in_memory_store_respects_runtime_pool_capacity() {
        let store = SessionStore::in_memory_with_config(RuntimeProfile {
            runtime_binding: "docker_runtime_pool".to_string(),
            compatibility_mode: "session_runtime_pool".to_string(),
            max_runtime_sessions: 2,
            supports_legacy_global_routes: false,
        });
        let alpha = principal("alpha");

        for _ in 0..2 {
            store
                .create_session(
                    &alpha,
                    CreateSessionRequest {
                        template_id: None,
                        owner_mode: None,
                        viewport: None,
                        idle_timeout_sec: None,
                        labels: HashMap::new(),
                        integration_context: None,
                    },
                    SessionOwnerMode::Collaborative,
                )
                .await
                .unwrap();
        }

        let error = store
            .create_session(
                &alpha,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: 2
            }
        ));
    }

    #[tokio::test]
    async fn in_memory_store_allows_delegated_client_to_load_session() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let delegate = service_principal("service-account-id", "bpane-mcp-bridge");

        let created = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let updated = store
            .set_automation_delegate_for_owner(
                &owner,
                created.id,
                SetAutomationDelegateRequest {
                    client_id: "bpane-mcp-bridge".to_string(),
                    issuer: None,
                    display_name: Some("BrowserPane MCP bridge".to_string()),
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            updated.automation_delegate.as_ref().unwrap().client_id,
            "bpane-mcp-bridge"
        );

        let visible = store
            .get_session_for_principal(&delegate, created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(visible.id, created.id);
    }

    #[tokio::test]
    async fn in_memory_store_stops_unused_ready_sessions_and_idle_sessions() {
        let store = SessionStore::in_memory();
        let owner = principal("owner");
        let created = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: Some(300),
                    labels: HashMap::new(),
                    integration_context: None,
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let stopped_ready = store
            .stop_session_if_idle(created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stopped_ready.state, SessionLifecycleState::Stopped);

        let created = store
            .create_session(
                &owner,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();

        let active = store
            .mark_session_active(created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(active.state, SessionLifecycleState::Active);

        let idle = store.mark_session_idle(created.id).await.unwrap().unwrap();
        assert_eq!(idle.state, SessionLifecycleState::Idle);

        let stopped = store
            .stop_session_if_idle(created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stopped.state, SessionLifecycleState::Stopped);

        let after = store
            .mark_session_active(created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.state, SessionLifecycleState::Stopped);
    }

    #[test]
    fn rejects_non_object_integration_context() {
        let error = validate_create_request(&CreateSessionRequest {
            template_id: None,
            owner_mode: None,
            viewport: None,
            idle_timeout_sec: None,
            labels: HashMap::new(),
            integration_context: Some(Value::String("bad".to_string())),
        })
        .unwrap_err();

        assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
    }
}
