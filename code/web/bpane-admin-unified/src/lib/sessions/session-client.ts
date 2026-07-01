import type {
  CreateSessionRequest,
  ProjectAdmissionDecision,
  SessionAutomationDelegate,
  SessionAccessTokenResponse,
  SessionBrowserContext,
  SessionCapabilities,
  SessionConnectInfo,
  SessionConnectionCounts,
  SessionConnectionInfo,
  SessionEffectiveEgress,
  SessionEgressDiagnostics,
  SessionIdleStatus,
  SessionListResponse,
  SessionNetworkIdentity,
  SessionProjectResource,
  SessionQueueInfo,
  SessionResource,
  SessionRuntimeInfo,
  SessionStatus,
  SessionStatusSummary,
  SessionStopEligibility,
  SessionViewport,
} from './session-types';

export type AccessTokenProvider = () => Promise<string | null> | string | null;
export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export type SessionCatalogErrorCode = 'missing_token' | 'http_error' | 'invalid_payload';

export type SessionCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class SessionCatalogError extends Error {
  readonly status: number | null;
  readonly code: SessionCatalogErrorCode;

  constructor(message: string, code: SessionCatalogErrorCode, status: number | null = null) {
    super(message);
    this.name = 'SessionCatalogError';
    this.code = code;
    this.status = status;
  }
}

export class SessionCatalogClient {
  readonly #baseUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider;
  readonly #fetchImpl: FetchLike;
  readonly #onAuthenticationFailure: (() => void) | undefined;

  constructor(options: SessionCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
    this.#fetchImpl = options.fetchImpl ?? fetch;
    this.#onAuthenticationFailure = options.onAuthenticationFailure;
  }

  async listSessions(): Promise<SessionListResponse> {
    const response = await this.#request(new URL('/api/v1/sessions', this.#baseUrl), {
      method: 'GET',
      headers: { accept: 'application/json' },
    });
    return toSessionListResponse(await response.json());
  }

  async createSession(request: CreateSessionRequest = {}): Promise<SessionResource> {
    const response = await this.#request(new URL('/api/v1/sessions', this.#baseUrl), {
      method: 'POST',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
      },
      body: JSON.stringify(request),
    });
    return toSessionResource(await response.json());
  }

  async getSession(sessionId: string): Promise<SessionResource> {
    const response = await this.#request(new URL(`/api/v1/sessions/${encodeURIComponent(sessionId)}`, this.#baseUrl), {
      method: 'GET',
      headers: { accept: 'application/json' },
    });
    return toSessionResource(await response.json());
  }

  async getSessionStatus(sessionId: string): Promise<SessionStatus> {
    const response = await this.#request(
      new URL(`/api/v1/sessions/${encodeURIComponent(sessionId)}/status`, this.#baseUrl),
      {
        method: 'GET',
        headers: { accept: 'application/json' },
      },
    );
    return toSessionStatus(await response.json());
  }

  async cancelQueuedSession(sessionId: string): Promise<SessionResource> {
    return await this.#sessionMutation(sessionId, 'cancel');
  }

  async releaseSessionRuntime(sessionId: string): Promise<SessionResource> {
    return await this.#sessionMutation(sessionId, 'release');
  }

  async stopSession(sessionId: string): Promise<SessionResource> {
    return await this.#sessionMutation(sessionId, 'stop');
  }

  async killSession(sessionId: string): Promise<SessionResource> {
    return await this.#sessionMutation(sessionId, 'kill');
  }

  async disconnectAllSessionConnections(sessionId: string): Promise<SessionStatus> {
    const response = await this.#request(
      new URL(`/api/v1/sessions/${encodeURIComponent(sessionId)}/connections/disconnect-all`, this.#baseUrl),
      {
        method: 'POST',
        headers: { accept: 'application/json' },
      },
    );
    return toSessionStatus(await response.json());
  }

  async issueSessionAccessToken(sessionId: string): Promise<SessionAccessTokenResponse> {
    const response = await this.#request(
      new URL(`/api/v1/sessions/${encodeURIComponent(sessionId)}/access-tokens`, this.#baseUrl),
      {
        method: 'POST',
        headers: { accept: 'application/json' },
      },
    );
    return toSessionAccessTokenResponse(await response.json());
  }

  async setAutomationDelegate(
    sessionId: string,
    delegate: SessionAutomationDelegate,
  ): Promise<SessionResource> {
    const response = await this.#request(
      new URL(`/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-owner`, this.#baseUrl),
      {
        method: 'POST',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        body: JSON.stringify(delegate),
      },
    );
    return toSessionResource(await response.json());
  }

  async clearAutomationDelegate(sessionId: string): Promise<SessionResource> {
    const response = await this.#request(
      new URL(`/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-owner`, this.#baseUrl),
      {
        method: 'DELETE',
        headers: { accept: 'application/json' },
      },
    );
    return toSessionResource(await response.json());
  }

  async #sessionMutation(sessionId: string, action: string): Promise<SessionResource> {
    const response = await this.#request(
      new URL(`/api/v1/sessions/${encodeURIComponent(sessionId)}/${action}`, this.#baseUrl),
      {
        method: 'POST',
        headers: { accept: 'application/json' },
      },
    );
    return toSessionResource(await response.json());
  }

  async #request(input: URL, init: RequestInit): Promise<Response> {
    const accessToken = await this.#accessTokenProvider();
    if (!accessToken) {
      throw new SessionCatalogError('No active admin access token is available.', 'missing_token');
    }

    const headers = new Headers(init.headers);
    headers.set('authorization', `Bearer ${accessToken}`);
    const response = await this.#fetchImpl(input, { ...init, headers });
    if (response.status === 401) {
      this.#onAuthenticationFailure?.();
    }
    if (!response.ok) {
      throw new SessionCatalogError(
        `Session catalog request failed with HTTP ${response.status}.`,
        'http_error',
        response.status,
      );
    }
    return response;
  }
}

export function toSessionListResponse(payload: unknown): SessionListResponse {
  const object = expectRecord(payload, 'session list response');
  return {
    sessions: expectArray(object.sessions, 'session list sessions').map(toSessionResource),
  };
}

export function toSessionResource(payload: unknown): SessionResource {
  const object = expectRecord(payload, 'session');
  return {
    id: expectString(object.id, 'session id'),
    state: expectString(object.state, 'session state'),
    project_id: optionalString(object.project_id, 'session project_id') ?? null,
    project: object.project === null || object.project === undefined ? null : toSessionProject(object.project),
    admission: object.admission === null || object.admission === undefined ? null : toProjectAdmission(object.admission),
    template_id: optionalString(object.template_id, 'session template_id') ?? null,
    browser_context: toSessionBrowserContext(object.browser_context),
    network_identity: object.network_identity === null || object.network_identity === undefined
      ? null
      : toSessionNetworkIdentity(object.network_identity),
    effective_egress: object.effective_egress === null || object.effective_egress === undefined
      ? null
      : toSessionEffectiveEgress(object.effective_egress),
    egress_diagnostics: object.egress_diagnostics === null || object.egress_diagnostics === undefined
      ? null
      : toSessionEgressDiagnostics(object.egress_diagnostics),
    owner_mode: optionalString(object.owner_mode, 'session owner_mode') ?? 'shared',
    viewport: object.viewport === null || object.viewport === undefined ? null : toViewport(object.viewport),
    capabilities: toSessionCapabilities(object.capabilities ?? {}),
    automation_delegate: object.automation_delegate === null || object.automation_delegate === undefined
      ? null
      : toAutomationDelegate(object.automation_delegate),
    idle_timeout_sec: optionalNumber(object.idle_timeout_sec, 'session idle_timeout_sec') ?? null,
    labels: toStringRecord(object.labels ?? {}, 'session labels'),
    integration_context: object.integration_context === null || object.integration_context === undefined
      ? null
      : toUnknownRecord(object.integration_context, 'session integration_context'),
    connect: toConnectInfo(object.connect),
    runtime: toRuntimeInfo(object.runtime),
    status: toSessionStatusSummary(object.status),
    queue: object.queue === null || object.queue === undefined ? null : toQueueInfo(object.queue),
    created_at: expectString(object.created_at, 'session created_at'),
    updated_at: expectString(object.updated_at, 'session updated_at'),
    queued_at: optionalString(object.queued_at, 'session queued_at') ?? null,
    runtime_released_at: optionalString(object.runtime_released_at, 'session runtime_released_at') ?? null,
    stopped_at: optionalString(object.stopped_at, 'session stopped_at') ?? null,
  };
}

export function toSessionStatus(payload: unknown): SessionStatus {
  const object = expectRecord(payload, 'session status');
  return {
    state: expectString(object.state, 'session status state'),
    project_id: optionalString(object.project_id, 'session status project_id') ?? null,
    project: object.project === null || object.project === undefined ? null : toSessionProject(object.project),
    admission: object.admission === null || object.admission === undefined ? null : toProjectAdmission(object.admission),
    runtime_state: expectString(object.runtime_state, 'session status runtime_state'),
    runtime_resume_mode: expectString(object.runtime_resume_mode, 'session status runtime_resume_mode'),
    presence_state: expectString(object.presence_state, 'session status presence_state'),
    connection_counts: toConnectionCounts(object.connection_counts),
    stop_eligibility: toStopEligibility(object.stop_eligibility),
    idle: toIdleStatus(object.idle ?? {}),
    connections: expectArray(object.connections, 'session status connections').map(toConnectionInfo),
    browser_clients: optionalNumber(object.browser_clients, 'session browser_clients') ?? 0,
    viewer_clients: optionalNumber(object.viewer_clients, 'session viewer_clients') ?? 0,
    recorder_clients: optionalNumber(object.recorder_clients, 'session recorder_clients') ?? 0,
    max_viewers: optionalNumber(object.max_viewers, 'session max_viewers') ?? 0,
    viewer_slots_remaining: optionalNumber(object.viewer_slots_remaining, 'session viewer_slots_remaining') ?? 0,
    exclusive_browser_owner: optionalBoolean(object.exclusive_browser_owner, 'session exclusive_browser_owner') ?? false,
    mcp_owner: optionalBoolean(object.mcp_owner, 'session mcp_owner') ?? false,
    resolution: toResolution(object.resolution),
    network_identity: object.network_identity === null || object.network_identity === undefined
      ? null
      : toSessionNetworkIdentity(object.network_identity),
    effective_egress: object.effective_egress === null || object.effective_egress === undefined
      ? null
      : toSessionEffectiveEgress(object.effective_egress),
    egress_diagnostics: object.egress_diagnostics === null || object.egress_diagnostics === undefined
      ? null
      : toSessionEgressDiagnostics(object.egress_diagnostics),
  };
}

export function toSessionAccessTokenResponse(payload: unknown): SessionAccessTokenResponse {
  const object = expectRecord(payload, 'session access token response');
  return {
    session_id: expectString(object.session_id, 'session access token session_id'),
    token_type: expectString(object.token_type, 'session access token token_type'),
    token: expectString(object.token, 'session access token token'),
    expires_at: expectString(object.expires_at, 'session access token expires_at'),
    connect: toConnectInfo(object.connect),
  };
}

function toSessionProject(payload: unknown): SessionProjectResource {
  const object = expectRecord(payload, 'session project');
  return {
    id: expectString(object.id, 'session project id'),
    name: expectString(object.name, 'session project name'),
    state: optionalString(object.state, 'session project state') ?? null,
  };
}

function toProjectAdmission(payload: unknown): ProjectAdmissionDecision {
  const object = expectRecord(payload, 'project admission');
  return {
    state: expectString(object.state, 'project admission state'),
    reason_code: optionalString(object.reason_code, 'project admission reason_code') ?? 'unknown',
    message: optionalString(object.message, 'project admission message') ?? '',
    checked_at: optionalString(object.checked_at, 'project admission checked_at') ?? '',
  };
}

function toSessionBrowserContext(payload: unknown): SessionBrowserContext {
  const object = expectRecord(payload, 'session browser_context');
  return {
    mode: optionalString(object.mode, 'session browser_context mode') ?? 'fresh',
    context_id: optionalString(object.context_id, 'session browser_context context_id') ?? null,
  };
}

function toSessionNetworkIdentity(payload: unknown): SessionNetworkIdentity {
  const object = expectRecord(payload, 'session network_identity');
  return {
    locale: optionalString(object.locale, 'session network locale') ?? null,
    languages: object.languages === undefined || object.languages === null
      ? []
      : expectArray(object.languages, 'session network languages').map((item) =>
          expectString(item, 'session network language'),
        ),
    timezone: optionalString(object.timezone, 'session network timezone') ?? null,
    user_agent: optionalString(object.user_agent, 'session network user_agent') ?? null,
    browser_identity: optionalString(object.browser_identity, 'session network browser_identity') ?? null,
    egress_profile_id: optionalString(object.egress_profile_id, 'session network egress_profile_id') ?? null,
  };
}

function toSessionEffectiveEgress(payload: unknown): SessionEffectiveEgress {
  const object = expectRecord(payload, 'session effective_egress');
  return {
    profile_id: optionalString(object.profile_id, 'effective egress profile_id') ?? null,
    profile_name: optionalString(object.profile_name, 'effective egress profile_name') ?? null,
    profile_state: optionalString(object.profile_state, 'effective egress profile_state') ?? null,
    proxy_configured: optionalBoolean(object.proxy_configured, 'effective egress proxy_configured') ?? false,
    proxy_auth_configured: optionalBoolean(object.proxy_auth_configured, 'effective egress proxy_auth_configured') ?? false,
    bypass_rule_count: optionalNumber(object.bypass_rule_count, 'effective egress bypass_rule_count') ?? 0,
    custom_ca_configured: optionalBoolean(object.custom_ca_configured, 'effective egress custom_ca_configured') ?? false,
    observation_mode: optionalString(object.observation_mode, 'effective egress observation_mode') ?? 'metadata_only',
    tls_interception_enabled:
      optionalBoolean(object.tls_interception_enabled, 'effective egress tls_interception_enabled') ?? false,
    sensitive_log_sink_configured:
      optionalBoolean(object.sensitive_log_sink_configured, 'effective egress sensitive_log_sink_configured') ?? false,
  };
}

function toSessionEgressDiagnostics(payload: unknown): SessionEgressDiagnostics {
  const object = expectRecord(payload, 'session egress_diagnostics');
  return {
    health: optionalString(object.health, 'session egress diagnostics health') ?? 'unknown',
    proof_level: optionalString(object.proof_level, 'session egress diagnostics proof_level') ?? 'none',
    observation_mode: optionalString(object.observation_mode, 'session egress diagnostics observation_mode') ?? 'metadata_only',
    warnings: object.warnings === undefined || object.warnings === null
      ? []
      : expectArray(object.warnings, 'session egress diagnostics warnings').map((item) =>
          expectString(item, 'session egress diagnostics warning'),
        ),
    observed_at: optionalString(object.observed_at, 'session egress diagnostics observed_at') ?? '',
  };
}

function toSessionCapabilities(payload: unknown): SessionCapabilities {
  const object = expectRecord(payload, 'session capabilities');
  return {
    browser_input: optionalBoolean(object.browser_input, 'session browser_input capability') ?? false,
    clipboard: optionalBoolean(object.clipboard, 'session clipboard capability') ?? false,
    audio: optionalBoolean(object.audio, 'session audio capability') ?? false,
    microphone: optionalBoolean(object.microphone, 'session microphone capability') ?? false,
    camera: optionalBoolean(object.camera, 'session camera capability') ?? false,
    file_transfer: optionalBoolean(object.file_transfer, 'session file_transfer capability') ?? false,
    resize: optionalBoolean(object.resize, 'session resize capability') ?? false,
  };
}

function toAutomationDelegate(payload: unknown): SessionAutomationDelegate {
  const object = expectRecord(payload, 'session automation_delegate');
  return {
    client_id: expectString(object.client_id, 'session automation_delegate client_id'),
    issuer: expectString(object.issuer, 'session automation_delegate issuer'),
    display_name: optionalString(object.display_name, 'session automation_delegate display_name') ?? null,
  };
}

function toViewport(payload: unknown): SessionViewport {
  const object = expectRecord(payload, 'session viewport');
  return {
    width: expectNumber(object.width, 'session viewport width'),
    height: expectNumber(object.height, 'session viewport height'),
  };
}

function toConnectInfo(payload: unknown): SessionConnectInfo {
  const object = expectRecord(payload, 'session connect');
  return {
    gateway_url: optionalString(object.gateway_url, 'session gateway_url') ?? '',
    transport_path: optionalString(object.transport_path, 'session transport_path') ?? '',
    auth_type: optionalString(object.auth_type, 'session auth_type') ?? '',
    ticket_path: optionalString(object.ticket_path, 'session ticket_path') ?? null,
    compatibility_mode: optionalString(object.compatibility_mode, 'session compatibility_mode') ?? '',
  };
}

function toRuntimeInfo(payload: unknown): SessionRuntimeInfo {
  const object = expectRecord(payload, 'session runtime');
  return {
    binding: optionalString(object.binding, 'session runtime binding') ?? '',
    compatibility_mode: optionalString(object.compatibility_mode, 'session runtime compatibility_mode') ?? '',
    cdp_endpoint: optionalString(object.cdp_endpoint, 'session runtime cdp_endpoint') ?? null,
  };
}

function toSessionStatusSummary(payload: unknown): SessionStatusSummary {
  const object = expectRecord(payload, 'session status summary');
  return {
    runtime_state: expectString(object.runtime_state, 'session summary runtime_state'),
    runtime_resume_mode: optionalString(object.runtime_resume_mode, 'session summary runtime_resume_mode') ?? 'profile_restart',
    presence_state: expectString(object.presence_state, 'session summary presence_state'),
    connection_counts: toConnectionCounts(object.connection_counts),
    stop_eligibility: toStopEligibility(object.stop_eligibility),
  };
}

function toQueueInfo(payload: unknown): SessionQueueInfo {
  const object = expectRecord(payload, 'session queue');
  return {
    queued_at: expectString(object.queued_at, 'session queue queued_at'),
    queued_for_ms: optionalNumber(object.queued_for_ms, 'session queue queued_for_ms') ?? 0,
    position: optionalNumber(object.position, 'session queue position') ?? 0,
    active_sessions: optionalNumber(object.active_sessions, 'session queue active_sessions') ?? 0,
    queued_sessions: optionalNumber(object.queued_sessions, 'session queue queued_sessions') ?? 0,
    max_active_sessions: optionalNumber(object.max_active_sessions, 'session queue max_active_sessions') ?? null,
    dispatch_blocker: optionalString(object.dispatch_blocker, 'session queue dispatch_blocker') ?? '',
    cancellable: optionalBoolean(object.cancellable, 'session queue cancellable') ?? true,
  };
}

function toConnectionCounts(payload: unknown): SessionConnectionCounts {
  const object = expectRecord(payload, 'session connection counts');
  return {
    interactive_clients: optionalNumber(object.interactive_clients, 'interactive_clients') ?? 0,
    owner_clients: optionalNumber(object.owner_clients, 'owner_clients') ?? 0,
    viewer_clients: optionalNumber(object.viewer_clients, 'viewer_clients') ?? 0,
    recorder_clients: optionalNumber(object.recorder_clients, 'recorder_clients') ?? 0,
    automation_clients: optionalNumber(object.automation_clients, 'automation_clients') ?? 0,
    total_clients: optionalNumber(object.total_clients, 'total_clients') ?? 0,
  };
}

function toStopEligibility(payload: unknown): SessionStopEligibility {
  const object = expectRecord(payload, 'session stop eligibility');
  return {
    allowed: optionalBoolean(object.allowed, 'session stop eligibility allowed') ?? false,
    blockers: object.blockers === undefined || object.blockers === null
      ? []
      : expectArray(object.blockers, 'session stop eligibility blockers').map((blocker) => {
          const blockerObject = expectRecord(blocker, 'session stop blocker');
          return {
            kind: expectString(blockerObject.kind, 'session stop blocker kind'),
            count: optionalNumber(blockerObject.count, 'session stop blocker count') ?? 0,
          };
        }),
  };
}

function toIdleStatus(payload: unknown): SessionIdleStatus {
  const object = expectRecord(payload, 'session idle');
  return {
    idle_timeout_sec: optionalNumber(object.idle_timeout_sec, 'idle_timeout_sec') ?? null,
    idle_since: optionalString(object.idle_since, 'idle_since') ?? null,
    idle_deadline: optionalString(object.idle_deadline, 'idle_deadline') ?? null,
  };
}

function toConnectionInfo(payload: unknown): SessionConnectionInfo {
  const object = expectRecord(payload, 'session connection');
  return {
    connection_id: expectNumber(object.connection_id, 'session connection_id'),
    role: expectString(object.role, 'session connection role'),
  };
}

function toResolution(payload: unknown): readonly [number, number] {
  if (!Array.isArray(payload) || payload.length < 2) {
    return [0, 0];
  }
  return [
    expectNumber(payload[0], 'session resolution width'),
    expectNumber(payload[1], 'session resolution height'),
  ];
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new SessionCatalogError(`${label} must be an object.`, 'invalid_payload');
  }
  return value as Record<string, unknown>;
}

function toUnknownRecord(value: unknown, label: string): Readonly<Record<string, unknown>> {
  return expectRecord(value, label);
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new SessionCatalogError(`${label} must be an array.`, 'invalid_payload');
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new SessionCatalogError(`${label} must be a non-empty string.`, 'invalid_payload');
  }
  return value;
}

function optionalString(value: unknown, label: string): string | undefined {
  if (value === undefined || value === null || value === '') {
    return undefined;
  }
  return expectString(value, label);
}

function expectNumber(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new SessionCatalogError(`${label} must be a finite number.`, 'invalid_payload');
  }
  return value;
}

function optionalNumber(value: unknown, label: string): number | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  return expectNumber(value, label);
}

function optionalBoolean(value: unknown, label: string): boolean | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  if (typeof value !== 'boolean') {
    throw new SessionCatalogError(`${label} must be a boolean.`, 'invalid_payload');
  }
  return value;
}

function toStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
  const object = expectRecord(value, label);
  return Object.fromEntries(
    Object.entries(object).map(([key, entry]) => [key, expectString(entry, `${label} ${key}`)]),
  );
}
