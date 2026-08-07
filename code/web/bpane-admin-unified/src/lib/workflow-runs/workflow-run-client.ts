import {
  AdminApiRequestError,
  AuthenticatedApiClient,
  formatAdminApiRequestError,
  type AccessTokenProvider,
  type AdminApiRequestErrorCode,
  type AdminApiRequestFailure,
  type FetchLike,
} from '$lib/api/authenticated-api';
import type {
  ProjectAdmissionDecision,
  SessionProjectResource,
} from '$lib/sessions/session-types';
import type {
  CreateWorkflowRunRequest,
  RejectWorkflowRunRequest,
  ResumeWorkflowRunRequest,
  SubmitWorkflowRunInputRequest,
  WorkflowRunAdmissionResource,
  WorkflowRunAppliedExtensionResource,
  WorkflowRunCredentialBindingResource,
  WorkflowRunEventListResponse,
  WorkflowRunEventResource,
  WorkflowRunInterventionRequestResource,
  WorkflowRunInterventionResolutionResource,
  WorkflowRunInterventionResource,
  WorkflowRunListResponse,
  WorkflowRunLogListResponse,
  WorkflowRunLogResource,
  WorkflowRunProducedFileListResponse,
  WorkflowRunProducedFileResource,
  WorkflowRunRecordingResource,
  WorkflowRunResource,
  WorkflowRunRetentionResource,
  WorkflowRunRuntimeResource,
  WorkflowRunSourceSnapshotResource,
  WorkflowRunWorkspaceInputResource,
} from './workflow-run-types';
import type { WorkflowSourceResource } from '$lib/workflows/workflow-types';

export type { AccessTokenProvider, FetchLike } from '$lib/api/authenticated-api';
export type WorkflowRunCatalogErrorCode = AdminApiRequestErrorCode;

export type WorkflowRunCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class WorkflowRunCatalogError extends AdminApiRequestError {
  constructor(
    message: string,
    code: WorkflowRunCatalogErrorCode,
    status: number | null = null,
    failure?: AdminApiRequestFailure,
  ) {
    super(message, failure ?? { code, status, message });
    this.name = 'WorkflowRunCatalogError';
  }
}

export class WorkflowRunCatalogClient {
  readonly #baseUrl: URL;
  readonly #api: AuthenticatedApiClient;

  constructor(options: WorkflowRunCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#api = new AuthenticatedApiClient({
      baseUrl: this.#baseUrl,
      accessTokenProvider: options.accessTokenProvider,
      ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
      ...(options.onAuthenticationFailure === undefined
        ? {}
        : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) => new WorkflowRunCatalogError(
        formatAdminApiRequestError('Workflow run catalog request', failure),
        failure.code,
        failure.status,
        failure,
      ),
    });
  }

  async listRuns(): Promise<WorkflowRunListResponse> {
    const response = await this.#request(new URL('/api/v1/workflow-runs', this.#baseUrl), {
      method: 'GET',
      headers: { accept: 'application/json' },
    });
    return toWorkflowRunListResponse(await response.json());
  }

  async createRun(request: CreateWorkflowRunRequest): Promise<WorkflowRunResource> {
    const response = await this.#request(new URL('/api/v1/workflow-runs', this.#baseUrl), {
      method: 'POST',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
      },
      body: JSON.stringify(request),
    });
    return toWorkflowRunResource(await response.json());
  }

  async getRun(runId: string): Promise<WorkflowRunResource> {
    return await this.#requestRun('GET', `/api/v1/workflow-runs/${encodeURIComponent(runId)}`);
  }

  async cancelRun(runId: string): Promise<WorkflowRunResource> {
    return await this.#requestRun('POST', `/api/v1/workflow-runs/${encodeURIComponent(runId)}/cancel`);
  }

  async resumeRun(
    runId: string,
    request: ResumeWorkflowRunRequest = {},
  ): Promise<WorkflowRunResource> {
    return await this.#requestRun(
      'POST',
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/resume`,
      request,
    );
  }

  async submitRunInput(
    runId: string,
    request: SubmitWorkflowRunInputRequest,
  ): Promise<WorkflowRunResource> {
    return await this.#requestRun(
      'POST',
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/submit-input`,
      request,
    );
  }

  async rejectRun(
    runId: string,
    request: RejectWorkflowRunRequest,
  ): Promise<WorkflowRunResource> {
    return await this.#requestRun(
      'POST',
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/reject`,
      request,
    );
  }

  async listRunEvents(runId: string): Promise<WorkflowRunEventListResponse> {
    const response = await this.#request(
      new URL(`/api/v1/workflow-runs/${encodeURIComponent(runId)}/events`, this.#baseUrl),
      { method: 'GET', headers: { accept: 'application/json' } },
    );
    return toWorkflowRunEventListResponse(await response.json());
  }

  async listRunLogs(runId: string): Promise<WorkflowRunLogListResponse> {
    const response = await this.#request(
      new URL(`/api/v1/workflow-runs/${encodeURIComponent(runId)}/logs`, this.#baseUrl),
      { method: 'GET', headers: { accept: 'application/json' } },
    );
    return toWorkflowRunLogListResponse(await response.json());
  }

  async listProducedFiles(runId: string): Promise<WorkflowRunProducedFileListResponse> {
    const response = await this.#request(
      new URL(`/api/v1/workflow-runs/${encodeURIComponent(runId)}/produced-files`, this.#baseUrl),
      { method: 'GET', headers: { accept: 'application/json' } },
    );
    return toWorkflowRunProducedFileListResponse(await response.json());
  }

  async downloadProducedFileContent(runId: string, fileId: string): Promise<Blob> {
    const response = await this.#request(
      new URL(
        `/api/v1/workflow-runs/${encodeURIComponent(runId)}/produced-files/${encodeURIComponent(fileId)}/content`,
        this.#baseUrl,
      ),
      { method: 'GET', headers: { accept: '*/*' } },
    );
    return await response.blob();
  }

  async #requestRun(method: 'GET' | 'POST', path: string, body?: unknown): Promise<WorkflowRunResource> {
    const response = await this.#request(new URL(path, this.#baseUrl), {
      method,
      headers: {
        accept: 'application/json',
        ...(body === undefined ? {} : { 'content-type': 'application/json' }),
      },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    });
    return toWorkflowRunResource(await response.json());
  }

  async #request(input: URL, init: RequestInit): Promise<Response> {
    return await this.#api.request(input, init);
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
    source_snapshot: toSourceSnapshot(object.source_snapshot),
    extensions: expectArray(object.extensions, 'workflow run extensions').map(toAppliedExtension),
    credential_bindings: expectArray(
      object.credential_bindings,
      'workflow run credential_bindings',
    ).map(toCredentialBinding),
    workspace_inputs: expectArray(object.workspace_inputs, 'workflow run workspace_inputs')
      .map(toWorkspaceInput),
    produced_files: expectArray(object.produced_files, 'workflow run produced_files').map(toProducedFile),
    recordings: expectArray(object.recordings, 'workflow run recordings').map(toRecording),
    retention: toRetention(object.retention),
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

export function toWorkflowRunEventListResponse(payload: unknown): WorkflowRunEventListResponse {
  const object = expectRecord(payload, 'workflow run event list response');
  return {
    events: expectArray(object.events, 'workflow run event list events').map(toEvent),
  };
}

export function toWorkflowRunLogListResponse(payload: unknown): WorkflowRunLogListResponse {
  const object = expectRecord(payload, 'workflow run log list response');
  return {
    logs: expectArray(object.logs, 'workflow run log list logs').map(toLog),
  };
}

export function toWorkflowRunProducedFileListResponse(
  payload: unknown,
): WorkflowRunProducedFileListResponse {
  const object = expectRecord(payload, 'workflow run produced file list response');
  return {
    files: expectArray(object.files, 'workflow run produced file list files').map(toProducedFile),
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
    project_id: optionalString(
      object.project_id,
      'workflow run project admission project_id',
    ) ?? null,
    active_sessions: optionalNumber(
      object.active_sessions,
      'workflow run project admission active_sessions',
    ) ?? null,
    max_active_sessions: optionalNumber(
      object.max_active_sessions,
      'workflow run project admission max_active_sessions',
    ) ?? null,
    active_workflow_runs: optionalNumber(
      object.active_workflow_runs,
      'workflow run project admission active_workflow_runs',
    ) ?? null,
    max_active_workflow_runs: optionalNumber(
      object.max_active_workflow_runs,
      'workflow run project admission max_active_workflow_runs',
    ) ?? null,
    session_creations: optionalNumber(
      object.session_creations,
      'workflow run project admission session_creations',
    ) ?? null,
    max_session_creations: optionalNumber(
      object.max_session_creations,
      'workflow run project admission max_session_creations',
    ) ?? null,
    session_creations_in_window: optionalNumber(
      object.session_creations_in_window,
      'workflow run project admission session_creations_in_window',
    ) ?? null,
    max_session_creations_per_window: optionalNumber(
      object.max_session_creations_per_window,
      'workflow run project admission max_session_creations_per_window',
    ) ?? null,
    session_creation_window_sec: optionalNumber(
      object.session_creation_window_sec,
      'workflow run project admission session_creation_window_sec',
    ) ?? null,
    runtime_usage_ms: optionalNumber(
      object.runtime_usage_ms,
      'workflow run project admission runtime_usage_ms',
    ) ?? null,
    max_runtime_usage_ms: optionalNumber(
      object.max_runtime_usage_ms,
      'workflow run project admission max_runtime_usage_ms',
    ) ?? null,
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
  const object = expectRecord(value, 'workflow run intervention');
  const pendingRequest = object.pending_request === undefined || object.pending_request === null
    ? object.pending_request
    : toInterventionRequest(object.pending_request);
  const lastResolution = object.last_resolution === undefined || object.last_resolution === null
    ? object.last_resolution
    : toInterventionResolution(object.last_resolution);
  return {
    ...(pendingRequest !== undefined ? { pending_request: pendingRequest } : {}),
    ...(lastResolution !== undefined ? { last_resolution: lastResolution } : {}),
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

function toInterventionResolution(value: unknown): WorkflowRunInterventionResolutionResource {
  const object = expectRecord(value, 'workflow run intervention last_resolution');
  return {
    request_id: optionalString(
      object.request_id,
      'workflow run intervention resolution request_id',
    ) ?? null,
    action: expectString(object.action, 'workflow run intervention resolution action'),
    ...(object.input !== undefined ? { input: object.input } : {}),
    reason: optionalString(object.reason, 'workflow run intervention resolution reason') ?? null,
    actor_subject: expectString(
      object.actor_subject,
      'workflow run intervention resolution actor_subject',
    ),
    actor_issuer: expectString(
      object.actor_issuer,
      'workflow run intervention resolution actor_issuer',
    ),
    actor_display_name: optionalString(
      object.actor_display_name,
      'workflow run intervention resolution actor_display_name',
    ) ?? null,
    ...(object.details !== undefined ? { details: object.details } : {}),
    resolved_at: expectString(
      object.resolved_at,
      'workflow run intervention resolution resolved_at',
    ),
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

function toSourceSnapshot(value: unknown): WorkflowRunSourceSnapshotResource | null {
  if (value === null) {
    return null;
  }
  const object = expectRecord(value, 'workflow run source_snapshot');
  return {
    source: toWorkflowSource(object.source),
    entrypoint: expectString(object.entrypoint, 'workflow run source_snapshot entrypoint'),
    workspace_id: expectString(object.workspace_id, 'workflow run source_snapshot workspace_id'),
    file_id: expectString(object.file_id, 'workflow run source_snapshot file_id'),
    file_name: expectString(object.file_name, 'workflow run source_snapshot file_name'),
    media_type: optionalString(object.media_type, 'workflow run source_snapshot media_type') ?? null,
    content_path: expectString(object.content_path, 'workflow run source_snapshot content_path'),
  };
}

function toWorkflowSource(value: unknown): WorkflowSourceResource {
  const object = expectRecord(value, 'workflow run source');
  const kind = expectString(object.kind, 'workflow run source kind');
  if (kind !== 'git') {
    throw new WorkflowRunCatalogError(
      `workflow run source kind must be git, got ${kind}.`,
      'invalid_payload',
    );
  }
  return {
    kind,
    repository_url: expectString(object.repository_url, 'workflow run source repository_url'),
    ref: optionalString(object.ref, 'workflow run source ref') ?? null,
    resolved_commit: optionalString(
      object.resolved_commit,
      'workflow run source resolved_commit',
    ) ?? null,
    root_path: optionalString(object.root_path, 'workflow run source root_path') ?? null,
  };
}

function toAppliedExtension(value: unknown): WorkflowRunAppliedExtensionResource {
  const object = expectRecord(value, 'workflow run extension');
  return {
    extension_id: expectString(object.extension_id, 'workflow run extension extension_id'),
    extension_version_id: expectString(
      object.extension_version_id,
      'workflow run extension extension_version_id',
    ),
    name: expectString(object.name, 'workflow run extension name'),
    version: expectString(object.version, 'workflow run extension version'),
  };
}

function toCredentialBinding(value: unknown): WorkflowRunCredentialBindingResource {
  const object = expectRecord(value, 'workflow run credential binding');
  return {
    id: expectString(object.id, 'workflow run credential binding id'),
    project_id: optionalString(
      object.project_id,
      'workflow run credential binding project_id',
    ) ?? null,
    name: expectString(object.name, 'workflow run credential binding name'),
    provider: expectString(object.provider, 'workflow run credential binding provider'),
    namespace: optionalString(
      object.namespace,
      'workflow run credential binding namespace',
    ) ?? null,
    allowed_origins: expectStringArray(
      object.allowed_origins,
      'workflow run credential binding allowed_origins',
    ),
    injection_mode: expectString(
      object.injection_mode,
      'workflow run credential binding injection_mode',
    ),
    totp: object.totp,
    resolve_path: expectString(
      object.resolve_path,
      'workflow run credential binding resolve_path',
    ),
  };
}

function toWorkspaceInput(value: unknown): WorkflowRunWorkspaceInputResource {
  const object = expectRecord(value, 'workflow run workspace input');
  return {
    id: expectString(object.id, 'workflow run workspace input id'),
    workspace_id: expectString(object.workspace_id, 'workflow run workspace input workspace_id'),
    file_id: expectString(object.file_id, 'workflow run workspace input file_id'),
    file_name: expectString(object.file_name, 'workflow run workspace input file_name'),
    media_type: optionalString(object.media_type, 'workflow run workspace input media_type') ?? null,
    byte_count: expectNumber(object.byte_count, 'workflow run workspace input byte_count'),
    sha256_hex: expectString(object.sha256_hex, 'workflow run workspace input sha256_hex'),
    ...(object.provenance !== undefined ? { provenance: object.provenance } : {}),
    mount_path: expectString(object.mount_path, 'workflow run workspace input mount_path'),
    content_path: expectString(object.content_path, 'workflow run workspace input content_path'),
  };
}

function toRecording(value: unknown): WorkflowRunRecordingResource {
  const object = expectRecord(value, 'workflow run recording');
  return {
    id: expectString(object.id, 'workflow run recording id'),
    session_id: expectString(object.session_id, 'workflow run recording session_id'),
    state: expectString(object.state, 'workflow run recording state'),
    format: expectString(object.format, 'workflow run recording format'),
    mime_type: optionalString(object.mime_type, 'workflow run recording mime_type') ?? null,
    bytes: optionalNumber(object.bytes, 'workflow run recording bytes') ?? null,
    duration_ms: optionalNumber(object.duration_ms, 'workflow run recording duration_ms') ?? null,
    error: optionalString(object.error, 'workflow run recording error') ?? null,
    termination_reason: optionalString(
      object.termination_reason,
      'workflow run recording termination_reason',
    ) ?? null,
    previous_recording_id: optionalString(
      object.previous_recording_id,
      'workflow run recording previous_recording_id',
    ) ?? null,
    started_at: expectString(object.started_at, 'workflow run recording started_at'),
    completed_at: optionalString(object.completed_at, 'workflow run recording completed_at') ?? null,
    content_path: expectString(object.content_path, 'workflow run recording content_path'),
    created_at: expectString(object.created_at, 'workflow run recording created_at'),
    updated_at: expectString(object.updated_at, 'workflow run recording updated_at'),
  };
}

function toRetention(value: unknown): WorkflowRunRetentionResource {
  const object = expectRecord(value, 'workflow run retention');
  return {
    logs_expire_at: optionalString(
      object.logs_expire_at,
      'workflow run retention logs_expire_at',
    ) ?? null,
    output_expire_at: optionalString(
      object.output_expire_at,
      'workflow run retention output_expire_at',
    ) ?? null,
  };
}

function toEvent(value: unknown): WorkflowRunEventResource {
  const object = expectRecord(value, 'workflow run event');
  return {
    id: expectString(object.id, 'workflow run event id'),
    run_id: expectString(object.run_id, 'workflow run event run_id'),
    source: expectString(object.source, 'workflow run event source'),
    automation_task_id: optionalString(
      object.automation_task_id,
      'workflow run event automation_task_id',
    ) ?? null,
    event_type: expectString(object.event_type, 'workflow run event event_type'),
    message: expectText(object.message, 'workflow run event message'),
    ...(object.data !== undefined ? { data: object.data } : {}),
    created_at: expectString(object.created_at, 'workflow run event created_at'),
  };
}

function toLog(value: unknown): WorkflowRunLogResource {
  const object = expectRecord(value, 'workflow run log');
  return {
    id: expectString(object.id, 'workflow run log id'),
    run_id: expectString(object.run_id, 'workflow run log run_id'),
    source: expectString(object.source, 'workflow run log source'),
    automation_task_id: optionalString(
      object.automation_task_id,
      'workflow run log automation_task_id',
    ) ?? null,
    stream: expectString(object.stream, 'workflow run log stream'),
    message: expectText(object.message, 'workflow run log message'),
    created_at: expectString(object.created_at, 'workflow run log created_at'),
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

function expectText(value: unknown, label: string): string {
  if (typeof value !== 'string') {
    throw new WorkflowRunCatalogError(`${label} must be a string.`, 'invalid_payload');
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

function optionalNumber(value: unknown, label: string): number | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  return expectNumber(value, label);
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
