import type {
  ProjectListResponse,
  ProjectPolicyOption,
  ProjectPolicyOptions,
  ProjectPolicy,
  ProjectQuotas,
  ProjectResource,
  ProjectState,
  ProjectUsageAlertMetric,
  ProjectUsageAlertResource,
  ProjectUsageAlertState,
  ProjectUsageBudgetEnforcement,
  ProjectUsageResource,
  UpsertProjectRequest,
} from './project-types';

const PROJECT_STATES = ['active', 'archived'] satisfies readonly ProjectState[];
const BUDGET_ENFORCEMENT = [
  'warning_only',
  'block_session_creation',
] satisfies readonly ProjectUsageBudgetEnforcement[];
const ALERT_METRICS = [
  'session_creations',
  'runtime_usage_ms',
  'egress_total_bytes',
] satisfies readonly ProjectUsageAlertMetric[];
const ALERT_STATES = ['approaching_limit', 'exceeded'] satisfies readonly ProjectUsageAlertState[];

export type AccessTokenProvider = () => Promise<string | null> | string | null;
export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export type ProjectCatalogErrorCode = 'missing_token' | 'http_error' | 'invalid_payload';

export type ProjectCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class ProjectCatalogError extends Error {
  readonly status: number | null;
  readonly code: ProjectCatalogErrorCode;

  constructor(message: string, code: ProjectCatalogErrorCode, status: number | null = null) {
    super(message);
    this.name = 'ProjectCatalogError';
    this.code = code;
    this.status = status;
  }
}

export class ProjectCatalogClient {
  readonly #baseUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider;
  readonly #fetchImpl: FetchLike;
  readonly #onAuthenticationFailure: (() => void) | undefined;

  constructor(options: ProjectCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
    this.#fetchImpl = options.fetchImpl ?? fetch;
    this.#onAuthenticationFailure = options.onAuthenticationFailure;
  }

  async listProjects(): Promise<ProjectListResponse> {
    const response = await this.#request(new URL('/api/v1/projects', this.#baseUrl), {
      method: 'GET',
      headers: {
        accept: 'application/json',
      },
    });

    return toProjectListResponse(await response.json());
  }

  async getProject(projectId: string): Promise<ProjectResource> {
    const response = await this.#request(new URL(`/api/v1/projects/${encodeURIComponent(projectId)}`, this.#baseUrl), {
      method: 'GET',
      headers: {
        accept: 'application/json',
      },
    });

    return toProjectResource(await response.json());
  }

  async updateProject(projectId: string, request: UpsertProjectRequest): Promise<ProjectResource> {
    const response = await this.#request(new URL(`/api/v1/projects/${encodeURIComponent(projectId)}`, this.#baseUrl), {
      method: 'PUT',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    return toProjectResource(await response.json());
  }

  async getProjectUsage(projectId: string): Promise<ProjectUsageResource> {
    const response = await this.#request(
      new URL(`/api/v1/projects/${encodeURIComponent(projectId)}/usage`, this.#baseUrl),
      {
        method: 'GET',
        headers: {
          accept: 'application/json',
        },
      },
    );

    return toProjectUsageResource(await response.json(), projectId);
  }

  async listProjectPolicyOptions(): Promise<ProjectPolicyOptions> {
    const [sessionTemplates, browserContexts, egressProfiles, extensions, fileWorkspaces] = await Promise.all([
      this.#listPolicyOptions('/api/v1/session-templates', 'templates', 'session template'),
      this.#listPolicyOptions('/api/v1/browser-contexts', 'contexts', 'browser context'),
      this.#listPolicyOptions('/api/v1/egress-profiles', 'profiles', 'egress profile'),
      this.#listPolicyOptions('/api/v1/extensions', 'extensions', 'extension'),
      this.#listPolicyOptions('/api/v1/file-workspaces', 'workspaces', 'file workspace'),
    ]);

    return {
      sessionTemplates,
      browserContexts,
      egressProfiles,
      extensions,
      fileWorkspaces,
    };
  }

  async #listPolicyOptions(pathname: string, collectionKey: string, label: string): Promise<readonly ProjectPolicyOption[]> {
    const response = await this.#request(new URL(pathname, this.#baseUrl), {
      method: 'GET',
      headers: {
        accept: 'application/json',
      },
    });
    return toProjectPolicyOptionList(await response.json(), collectionKey, label);
  }

  async #request(input: URL, init: RequestInit): Promise<Response> {
    const accessToken = await this.#accessTokenProvider();
    if (!accessToken) {
      throw new ProjectCatalogError('No active admin access token is available.', 'missing_token');
    }

    const headers = new Headers(init.headers);
    headers.set('authorization', `Bearer ${accessToken}`);

    const response = await this.#fetchImpl(input, {
      ...init,
      headers,
    });
    if (response.status === 401) {
      this.#onAuthenticationFailure?.();
    }
    if (!response.ok) {
      throw new ProjectCatalogError(
        `Project catalog request failed with HTTP ${response.status}.`,
        'http_error',
        response.status,
      );
    }

    return response;
  }
}

export function toProjectListResponse(payload: unknown): ProjectListResponse {
  const object = expectRecord(payload, 'project list response');
  const projects = expectArray(object.projects, 'project list projects').map(toProjectResource);
  return { projects };
}

export function toProjectResource(value: unknown): ProjectResource {
  const object = expectRecord(value, 'project');
  const id = expectString(object.id, 'project id');
  return {
    id,
    name: expectString(object.name, 'project name'),
    description: optionalString(object.description, 'project description') ?? null,
    labels: toStringRecord(object.labels ?? {}, 'project labels'),
    quotas: toProjectQuotas(object.quotas ?? {}),
    policy: toProjectPolicy(object.policy ?? {}),
    state: expectEnum(object.state, PROJECT_STATES, 'project state'),
    usage: toProjectUsageResource(object.usage, id),
    created_at: expectString(object.created_at, 'project created_at'),
    updated_at: expectString(object.updated_at, 'project updated_at'),
  };
}

function toProjectQuotas(value: unknown): ProjectQuotas {
  const object = expectRecord(value, 'project quotas');
  return {
    max_active_sessions: optionalNumber(object.max_active_sessions, 'project max_active_sessions') ?? null,
    max_active_workflow_runs: optionalNumber(object.max_active_workflow_runs, 'project max_active_workflow_runs') ?? null,
    max_retained_storage_bytes: optionalNumber(object.max_retained_storage_bytes, 'project max_retained_storage_bytes') ?? null,
    max_session_creations: optionalNumber(object.max_session_creations, 'project max_session_creations') ?? null,
    max_session_creations_per_window:
      optionalNumber(object.max_session_creations_per_window, 'project max_session_creations_per_window') ?? null,
    session_creation_window_sec:
      optionalNumber(object.session_creation_window_sec, 'project session_creation_window_sec') ?? null,
    max_runtime_usage_ms: optionalNumber(object.max_runtime_usage_ms, 'project max_runtime_usage_ms') ?? null,
    max_egress_total_bytes: optionalNumber(object.max_egress_total_bytes, 'project max_egress_total_bytes') ?? null,
  };
}

function toProjectPolicy(value: unknown): ProjectPolicy {
  const object = expectRecord(value, 'project policy');
  return {
    allowed_session_template_ids: toStringArray(object.allowed_session_template_ids, 'allowed_session_template_ids'),
    allowed_egress_profile_ids: toStringArray(object.allowed_egress_profile_ids, 'allowed_egress_profile_ids'),
    allowed_extension_ids: toStringArray(object.allowed_extension_ids, 'allowed_extension_ids'),
    allowed_browser_context_ids: toStringArray(object.allowed_browser_context_ids, 'allowed_browser_context_ids'),
    allowed_file_workspace_ids: toStringArray(object.allowed_file_workspace_ids, 'allowed_file_workspace_ids'),
    allow_browser_uploads: optionalBoolean(object.allow_browser_uploads, 'allow_browser_uploads') ?? true,
    allow_browser_downloads: optionalBoolean(object.allow_browser_downloads, 'allow_browser_downloads') ?? true,
    allow_session_file_bindings: optionalBoolean(object.allow_session_file_bindings, 'allow_session_file_bindings') ?? true,
    allow_manual_recordings: optionalBoolean(object.allow_manual_recordings, 'allow_manual_recordings') ?? true,
    usage_budget_enforcement:
      optionalEnum(object.usage_budget_enforcement, BUDGET_ENFORCEMENT, 'usage_budget_enforcement') ?? 'warning_only',
  };
}

export function toProjectUsageResource(value: unknown, fallbackProjectId: string): ProjectUsageResource {
  const object = expectRecord(value, 'project usage');
  return {
    project_id: optionalString(object.project_id, 'project usage project_id') ?? fallbackProjectId,
    active_sessions: expectNumber(object.active_sessions, 'project active_sessions'),
    queued_sessions: expectNumber(object.queued_sessions, 'project queued_sessions'),
    session_creations: expectNumber(object.session_creations, 'project session_creations'),
    max_session_creations: optionalNumber(object.max_session_creations, 'project usage max_session_creations') ?? null,
    max_active_sessions: optionalNumber(object.max_active_sessions, 'project usage max_active_sessions') ?? null,
    active_workflow_runs: expectNumber(object.active_workflow_runs, 'project active_workflow_runs'),
    max_active_workflow_runs:
      optionalNumber(object.max_active_workflow_runs, 'project usage max_active_workflow_runs') ?? null,
    runtime_usage_ms: expectNumber(object.runtime_usage_ms, 'project runtime_usage_ms'),
    max_runtime_usage_ms: optionalNumber(object.max_runtime_usage_ms, 'project usage max_runtime_usage_ms') ?? null,
    egress_rx_bytes: expectNumber(object.egress_rx_bytes, 'project egress_rx_bytes'),
    egress_tx_bytes: expectNumber(object.egress_tx_bytes, 'project egress_tx_bytes'),
    egress_total_bytes: expectNumber(object.egress_total_bytes, 'project egress_total_bytes'),
    max_egress_total_bytes: optionalNumber(object.max_egress_total_bytes, 'project usage max_egress_total_bytes') ?? null,
    retained_storage_bytes: expectNumber(object.retained_storage_bytes, 'project retained_storage_bytes'),
    max_retained_storage_bytes:
      optionalNumber(object.max_retained_storage_bytes, 'project usage max_retained_storage_bytes') ?? null,
    alerts: expectArray(object.alerts, 'project usage alerts').map(toProjectUsageAlert),
    observed_at: expectString(object.observed_at, 'project usage observed_at'),
  };
}

export function toProjectPolicyOptionList(
  payload: unknown,
  collectionKey: string,
  label: string,
): readonly ProjectPolicyOption[] {
  const object = expectRecord(payload, `${label} list response`);
  return expectArray(object[collectionKey], `${label} list ${collectionKey}`).map((entry) =>
    toProjectPolicyOption(entry, label),
  );
}

function toProjectUsageAlert(value: unknown): ProjectUsageAlertResource {
  const object = expectRecord(value, 'project usage alert');
  return {
    metric: expectEnum(object.metric, ALERT_METRICS, 'project usage alert metric'),
    state: expectEnum(object.state, ALERT_STATES, 'project usage alert state'),
    current_value: expectNumber(object.current_value, 'project usage alert current_value'),
    limit_value: expectNumber(object.limit_value, 'project usage alert limit_value'),
    threshold_percent: expectNumber(object.threshold_percent, 'project usage alert threshold_percent'),
    message: expectString(object.message, 'project usage alert message'),
  };
}

function toProjectPolicyOption(value: unknown, label: string): ProjectPolicyOption {
  const object = expectRecord(value, label);
  return {
    id: expectString(object.id, `${label} id`),
    name: expectString(object.name, `${label} name`),
    description: optionalString(object.description, `${label} description`) ?? null,
    state: optionState(object),
    scope: optionScope(object),
  };
}

function optionState(object: Record<string, unknown>): string | null {
  const enabled = optionalBoolean(object.enabled, 'policy option enabled');
  if (enabled !== undefined) {
    return enabled ? 'enabled' : 'disabled';
  }
  const state = optionalString(object.state, 'policy option state');
  if (state) {
    return state;
  }
  const persistenceMode = optionalString(object.persistence_mode, 'policy option persistence_mode');
  if (persistenceMode) {
    return persistenceMode;
  }
  const version = optionalNumber(object.version, 'policy option version');
  if (version !== undefined) {
    return `v${version}`;
  }
  return null;
}

function optionScope(object: Record<string, unknown>): string | null {
  const project = object.project;
  if (project && typeof project === 'object' && !Array.isArray(project)) {
    const projectObject = project as Record<string, unknown>;
    const projectName = optionalString(projectObject.name, 'policy option project name');
    if (projectName) {
      return `project ${projectName}`;
    }
  }
  const projectId = optionalString(object.project_id, 'policy option project_id');
  if (projectId) {
    return `project ${projectId}`;
  }
  return 'owner scoped';
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new ProjectCatalogError(`${label} must be an object.`, 'invalid_payload');
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new ProjectCatalogError(`${label} must be an array.`, 'invalid_payload');
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new ProjectCatalogError(`${label} must be a non-empty string.`, 'invalid_payload');
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
    throw new ProjectCatalogError(`${label} must be a finite number.`, 'invalid_payload');
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
    throw new ProjectCatalogError(`${label} must be a boolean.`, 'invalid_payload');
  }
  return value;
}

function expectEnum<T extends string>(value: unknown, allowed: readonly T[], label: string): T {
  if (typeof value !== 'string' || !allowed.includes(value as T)) {
    throw new ProjectCatalogError(`${label} has an unsupported value.`, 'invalid_payload');
  }
  return value as T;
}

function optionalEnum<T extends string>(value: unknown, allowed: readonly T[], label: string): T | undefined {
  if (value === undefined || value === null || value === '') {
    return undefined;
  }
  return expectEnum(value, allowed, label);
}

function toStringArray(value: unknown, label: string): readonly string[] {
  if (value === undefined || value === null) {
    return [];
  }
  return expectArray(value, label).map((entry) => expectString(entry, `${label} entry`));
}

function toStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
  const object = expectRecord(value, label);
  return Object.fromEntries(
    Object.entries(object).map(([key, entry]) => [key, expectString(entry, `${label} ${key}`)]),
  );
}
