import type {
  ProjectAdmissionDecision,
  SessionProjectResource,
} from '$lib/sessions/session-types';
import type {
  WorkflowRunAdmissionResource,
  WorkflowRunInterventionRequestResource,
  WorkflowRunInterventionResource,
  WorkflowRunListResponse,
  WorkflowRunProducedFileResource,
  WorkflowRunResource,
  WorkflowRunRuntimeResource,
} from './workflow-run-types';

export type AccessTokenProvider = () => Promise<string | null> | string | null;
export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export type WorkflowRunCatalogErrorCode = 'missing_token' | 'http_error' | 'invalid_payload';

export type WorkflowRunCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class WorkflowRunCatalogError extends Error {
  readonly status: number | null;
  readonly code: WorkflowRunCatalogErrorCode;

  constructor(message: string, code: WorkflowRunCatalogErrorCode, status: number | null = null) {
    super(message);
    this.name = 'WorkflowRunCatalogError';
    this.code = code;
    this.status = status;
  }
}

export class WorkflowRunCatalogClient {
  readonly #baseUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider;
  readonly #fetchImpl: FetchLike;
  readonly #onAuthenticationFailure: (() => void) | undefined;

  constructor(options: WorkflowRunCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
    this.#fetchImpl = options.fetchImpl ?? fetch;
    this.#onAuthenticationFailure = options.onAuthenticationFailure;
  }

  async listRuns(): Promise<WorkflowRunListResponse> {
    const response = await this.#request(new URL('/api/v1/workflow-runs', this.#baseUrl), {
      method: 'GET',
      headers: { accept: 'application/json' },
    });
    return toWorkflowRunListResponse(await response.json());
  }

  async #request(input: URL, init: RequestInit): Promise<Response> {
    const accessToken = await this.#accessTokenProvider();
    if (!accessToken) {
      throw new WorkflowRunCatalogError('No active admin access token is available.', 'missing_token');
    }

    const headers = new Headers(init.headers);
    headers.set('authorization', `Bearer ${accessToken}`);

    const response = await this.#fetchImpl(input, { ...init, headers });
    if (response.status === 401) {
      this.#onAuthenticationFailure?.();
    }
    if (!response.ok) {
      throw new WorkflowRunCatalogError(
        `Workflow run catalog request failed with HTTP ${response.status}.`,
        'http_error',
        response.status,
      );
    }
    return response;
  }
}

export function toWorkflowRunListResponse(payload: unknown): WorkflowRunListResponse {
  const object = expectRecord(payload, 'workflow run list response');
  return {
    runs: expectArray(object.runs, 'workflow run list runs').map(toWorkflowRunResource),
  };
}

export function toWorkflowRunResource(payload: unknown): WorkflowRunResource {
  const object = expectRecord(payload, 'workflow run');
  return {
    id: expectString(object.id, 'workflow run id'),
    workflow_definition_id: expectString(object.workflow_definition_id, 'workflow run workflow_definition_id'),
    workflow_definition_version_id: expectString(
      object.workflow_definition_version_id,
      'workflow run workflow_definition_version_id',
    ),
    workflow_version: expectString(object.workflow_version, 'workflow run workflow_version'),
    project_id: optionalString(object.project_id, 'workflow run project_id') ?? null,
    project: toSessionProjectResource(object.project),
    source_system: optionalString(object.source_system, 'workflow run source_system') ?? null,
    source_reference: optionalString(object.source_reference, 'workflow run source_reference') ?? null,
    client_request_id: optionalString(object.client_request_id, 'workflow run client_request_id') ?? null,
    state: expectString(object.state, 'workflow run state'),
    session_id: expectString(object.session_id, 'workflow run session_id'),
    automation_task_id: expectString(object.automation_task_id, 'workflow run automation_task_id'),
    ...(object.input !== undefined ? { input: object.input } : {}),
    ...(object.output !== undefined ? { output: object.output } : {}),
    error: optionalString(object.error, 'workflow run error') ?? null,
    artifact_refs: expectStringArray(object.artifact_refs ?? [], 'workflow run artifact_refs'),
    produced_files: expectArray(object.produced_files ?? [], 'workflow run produced_files').map(toProducedFile),
    project_admission: toProjectAdmissionDecision(object.project_admission),
    admission: toAdmission(object.admission),
    intervention: toIntervention(object.intervention),
    runtime: toRuntime(object.runtime),
    labels: toStringRecord(object.labels ?? {}, 'workflow run labels'),
    started_at: optionalString(object.started_at, 'workflow run started_at') ?? null,
    completed_at: optionalString(object.completed_at, 'workflow run completed_at') ?? null,
    events_path: expectString(object.events_path, 'workflow run events_path'),
    logs_path: expectString(object.logs_path, 'workflow run logs_path'),
    created_at: expectString(object.created_at, 'workflow run created_at'),
    updated_at: expectString(object.updated_at, 'workflow run updated_at'),
  };
}

function toSessionProjectResource(value: unknown): SessionProjectResource | null {
  if (value === undefined || value === null) {
    return null;
  }
  const object = expectRecord(value, 'workflow run project');
  return {
    id: expectString(object.id, 'workflow run project id'),
    name: expectString(object.name, 'workflow run project name'),
    state: optionalString(object.state, 'workflow run project state') ?? null,
  };
}

function toProjectAdmissionDecision(value: unknown): ProjectAdmissionDecision | null {
  if (value === undefined || value === null) {
    return null;
  }
  const object = expectRecord(value, 'workflow run project admission');
  return {
    state: expectString(object.state, 'workflow run project admission state'),
    reason_code: expectString(object.reason_code, 'workflow run project admission reason_code'),
    message: expectString(object.message, 'workflow run project admission message'),
    checked_at: expectString(object.checked_at, 'workflow run project admission checked_at'),
  };
}

function toAdmission(value: unknown): WorkflowRunAdmissionResource | null {
  if (value === undefined || value === null) {
    return null;
  }
  const object = expectRecord(value, 'workflow run admission');
  return {
    state: expectString(object.state, 'workflow run admission state'),
    reason: expectString(object.reason, 'workflow run admission reason'),
    ...(object.details !== undefined ? { details: object.details } : {}),
    queued_at: expectString(object.queued_at, 'workflow run admission queued_at'),
  };
}

function toIntervention(value: unknown): WorkflowRunInterventionResource {
  if (value === undefined || value === null) {
    return {};
  }
  const object = expectRecord(value, 'workflow run intervention');
  const pendingRequest = object.pending_request === undefined || object.pending_request === null
    ? object.pending_request
    : toInterventionRequest(object.pending_request);
  return {
    ...(pendingRequest !== undefined ? { pending_request: pendingRequest } : {}),
  };
}

function toInterventionRequest(value: unknown): WorkflowRunInterventionRequestResource {
  const object = expectRecord(value, 'workflow run intervention pending_request');
  return {
    request_id: expectString(object.request_id, 'workflow run intervention request_id'),
    kind: expectString(object.kind, 'workflow run intervention kind'),
    prompt: optionalString(object.prompt, 'workflow run intervention prompt') ?? null,
    ...(object.details !== undefined ? { details: object.details } : {}),
    requested_at: expectString(object.requested_at, 'workflow run intervention requested_at'),
  };
}

function toRuntime(value: unknown): WorkflowRunRuntimeResource | null {
  if (value === undefined || value === null) {
    return null;
  }
  const object = expectRecord(value, 'workflow run runtime');
  return {
    resume_mode: expectString(object.resume_mode, 'workflow run runtime resume_mode'),
    exact_runtime_available: expectBoolean(
      object.exact_runtime_available,
      'workflow run runtime exact_runtime_available',
    ),
    hold_until: optionalString(object.hold_until, 'workflow run runtime hold_until') ?? null,
    released_at: optionalString(object.released_at, 'workflow run runtime released_at') ?? null,
    release_reason: optionalString(object.release_reason, 'workflow run runtime release_reason') ?? null,
    session_state: optionalString(object.session_state, 'workflow run runtime session_state') ?? null,
  };
}

function toProducedFile(value: unknown): WorkflowRunProducedFileResource {
  const object = expectRecord(value, 'workflow run produced file');
  return {
    workspace_id: expectString(object.workspace_id, 'workflow run produced file workspace_id'),
    file_id: expectString(object.file_id, 'workflow run produced file file_id'),
    file_name: expectString(object.file_name, 'workflow run produced file file_name'),
    media_type: optionalString(object.media_type, 'workflow run produced file media_type') ?? null,
    byte_count: expectNumber(object.byte_count, 'workflow run produced file byte_count'),
    sha256_hex: expectString(object.sha256_hex, 'workflow run produced file sha256_hex'),
    ...(object.provenance !== undefined ? { provenance: object.provenance } : {}),
    content_path: expectString(object.content_path, 'workflow run produced file content_path'),
    created_at: expectString(object.created_at, 'workflow run produced file created_at'),
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new WorkflowRunCatalogError(`${label} must be an object.`, 'invalid_payload');
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new WorkflowRunCatalogError(`${label} must be an array.`, 'invalid_payload');
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new WorkflowRunCatalogError(`${label} must be a non-empty string.`, 'invalid_payload');
  }
  return value;
}

function optionalString(value: unknown, label: string): string | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  if (typeof value !== 'string') {
    throw new WorkflowRunCatalogError(`${label} must be a string.`, 'invalid_payload');
  }
  return value;
}

function expectNumber(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new WorkflowRunCatalogError(`${label} must be a finite number.`, 'invalid_payload');
  }
  return value;
}

function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') {
    throw new WorkflowRunCatalogError(`${label} must be a boolean.`, 'invalid_payload');
  }
  return value;
}

function expectStringArray(value: unknown, label: string): readonly string[] {
  return expectArray(value, label).map((entry) => expectString(entry, `${label} entry`));
}

function toStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
  const object = expectRecord(value, label);
  return Object.fromEntries(
    Object.entries(object).map(([key, item]) => [key, expectString(item, `${label}.${key}`)]),
  );
}
