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
  UpsertWorkflowEndpointGrantRequest,
  UpsertWorkflowEndpointRequest,
  WorkflowEndpointGrantListResponse,
  WorkflowEndpointGrantOperation,
  WorkflowEndpointGrantResource,
  WorkflowEndpointListResponse,
  WorkflowEndpointResource,
  WorkflowEndpointState,
} from './workflow-endpoint-types';

const ENDPOINT_STATES = ['draft', 'active', 'disabled'] satisfies readonly WorkflowEndpointState[];
const GRANT_OPERATIONS = [
  'invoke',
  'read',
  'cancel',
  'artifact.read',
] satisfies readonly WorkflowEndpointGrantOperation[];

export class WorkflowEndpointCatalogError extends AdminApiRequestError {
  constructor(
    message: string,
    code: AdminApiRequestErrorCode,
    status: number | null = null,
    failure?: AdminApiRequestFailure,
  ) {
    super(message, failure ?? { code, status, message });
    this.name = 'WorkflowEndpointCatalogError';
  }
}

export class WorkflowEndpointCatalogClient {
  readonly #baseUrl: URL;
  readonly #api: AuthenticatedApiClient;

  constructor(options: {
    readonly baseUrl: string | URL;
    readonly accessTokenProvider: AccessTokenProvider;
    readonly fetchImpl?: FetchLike;
    readonly onAuthenticationFailure?: () => void;
  }) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#api = new AuthenticatedApiClient({
      baseUrl: this.#baseUrl,
      accessTokenProvider: options.accessTokenProvider,
      ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
      ...(options.onAuthenticationFailure === undefined
        ? {}
        : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) =>
        new WorkflowEndpointCatalogError(
          formatAdminApiRequestError('Workflow endpoint request', failure),
          failure.code,
          failure.status,
          failure,
        ),
    });
  }

  async listEndpoints(projectId: string): Promise<WorkflowEndpointListResponse> {
    return toEndpointList(await (await this.#request(projectPath(projectId), { method: 'GET' })).json());
  }

  async createEndpoint(
    projectId: string,
    request: UpsertWorkflowEndpointRequest,
  ): Promise<WorkflowEndpointResource> {
    return toEndpoint(await (await this.#request(projectPath(projectId), jsonRequest('POST', request))).json());
  }

  async getEndpoint(projectId: string, endpointKey: string): Promise<WorkflowEndpointResource> {
    return toEndpoint(await (await this.#request(endpointPath(projectId, endpointKey), { method: 'GET' })).json());
  }

  async updateEndpoint(
    projectId: string,
    endpointKey: string,
    request: UpsertWorkflowEndpointRequest,
  ): Promise<WorkflowEndpointResource> {
    return toEndpoint(
      await (await this.#request(endpointPath(projectId, endpointKey), jsonRequest('PUT', request))).json(),
    );
  }

  async activateEndpoint(projectId: string, endpointKey: string): Promise<WorkflowEndpointResource> {
    return this.#transition(projectId, endpointKey, 'activate');
  }

  async disableEndpoint(projectId: string, endpointKey: string): Promise<WorkflowEndpointResource> {
    return this.#transition(projectId, endpointKey, 'disable');
  }

  async listGrants(projectId: string, endpointKey: string): Promise<WorkflowEndpointGrantListResponse> {
    return toGrantList(
      await (await this.#request(`${endpointPath(projectId, endpointKey)}/grants`, { method: 'GET' })).json(),
    );
  }

  async upsertGrant(
    projectId: string,
    endpointKey: string,
    request: UpsertWorkflowEndpointGrantRequest,
  ): Promise<WorkflowEndpointGrantResource> {
    return toGrant(
      await (
        await this.#request(`${endpointPath(projectId, endpointKey)}/grants`, jsonRequest('POST', request))
      ).json(),
    );
  }

  async revokeGrant(projectId: string, endpointKey: string, grantId: string): Promise<void> {
    await this.#request(
      `${endpointPath(projectId, endpointKey)}/grants/${encodeURIComponent(grantId)}`,
      { method: 'DELETE' },
    );
  }

  async #transition(projectId: string, endpointKey: string, transition: 'activate' | 'disable') {
    return toEndpoint(
      await (
        await this.#request(`${endpointPath(projectId, endpointKey)}/${transition}`, { method: 'POST' })
      ).json(),
    );
  }

  async #request(path: string, init: RequestInit): Promise<Response> {
    const headers = new Headers(init.headers);
    headers.set('accept', 'application/json');
    return this.#api.request(new URL(path, this.#baseUrl), { ...init, headers });
  }
}

function projectPath(projectId: string): string {
  return `/api/v1/projects/${encodeURIComponent(projectId)}/workflow-endpoints`;
}

function endpointPath(projectId: string, endpointKey: string): string {
  return `${projectPath(projectId)}/${encodeURIComponent(endpointKey)}`;
}

function jsonRequest(method: string, body: unknown): RequestInit {
  return {
    method,
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  };
}

function toEndpointList(value: unknown): WorkflowEndpointListResponse {
  const object = expectRecord(value, 'workflow endpoint list');
  return {
    workflow_endpoints: expectArray(object.workflow_endpoints, 'workflow endpoints').map(toEndpoint),
  };
}

function toEndpoint(value: unknown): WorkflowEndpointResource {
  const object = expectRecord(value, 'workflow endpoint');
  const artifact = expectRecord(object.artifact_behavior, 'workflow endpoint artifact_behavior');
  const labels = expectRecord(object.labels, 'workflow endpoint labels');
  if (artifact.mode !== 'authorized_references') {
    throw invalid('workflow endpoint artifact_behavior.mode must be authorized_references.');
  }
  return {
    id: expectString(object.id, 'workflow endpoint id'),
    project_id: expectString(object.project_id, 'workflow endpoint project_id'),
    endpoint_key: expectString(object.endpoint_key, 'workflow endpoint endpoint_key'),
    purpose: expectString(object.purpose, 'workflow endpoint purpose'),
    workflow_definition_id: expectString(object.workflow_definition_id, 'workflow endpoint workflow_definition_id'),
    workflow_definition_version_id: expectString(object.workflow_definition_version_id, 'workflow endpoint workflow_definition_version_id'),
    workflow_version: expectString(object.workflow_version, 'workflow endpoint workflow_version'),
    input_schema: expectPresent(object.input_schema, 'workflow endpoint input_schema'),
    output_schema: expectPresent(object.output_schema, 'workflow endpoint output_schema'),
    execution_timeout_seconds: expectNumber(object.execution_timeout_seconds, 'workflow endpoint execution_timeout_seconds'),
    inline_result_max_bytes: expectNumber(object.inline_result_max_bytes, 'workflow endpoint inline_result_max_bytes'),
    artifact_behavior: {
      mode: 'authorized_references',
      retention_seconds: expectNumber(artifact.retention_seconds, 'workflow endpoint artifact retention_seconds'),
    },
    supported_controls: expectArray(object.supported_controls, 'workflow endpoint supported_controls').map((item) => expectString(item, 'workflow endpoint supported control')),
    labels: Object.fromEntries(
      Object.entries(labels).map(([key, labelValue]) => [key, expectString(labelValue, `workflow endpoint label ${key}`)]),
    ),
    state: expectEnum(object.state, ENDPOINT_STATES, 'workflow endpoint state'),
    grants_path: expectString(object.grants_path, 'workflow endpoint grants_path'),
    invocations_path: expectString(object.invocations_path, 'workflow endpoint invocations_path'),
    created_at: expectString(object.created_at, 'workflow endpoint created_at'),
    updated_at: expectString(object.updated_at, 'workflow endpoint updated_at'),
  };
}

function toGrantList(value: unknown): WorkflowEndpointGrantListResponse {
  const object = expectRecord(value, 'workflow endpoint grant list');
  return { grants: expectArray(object.grants, 'workflow endpoint grants').map(toGrant) };
}

function toGrant(value: unknown): WorkflowEndpointGrantResource {
  const object = expectRecord(value, 'workflow endpoint grant');
  return {
    id: expectString(object.id, 'workflow endpoint grant id'),
    endpoint_id: expectString(object.endpoint_id, 'workflow endpoint grant endpoint_id'),
    project_id: expectString(object.project_id, 'workflow endpoint grant project_id'),
    service_principal_id: expectString(object.service_principal_id, 'workflow endpoint grant service_principal_id'),
    operations: expectArray(object.operations, 'workflow endpoint grant operations').map((item) =>
      expectEnum(item, GRANT_OPERATIONS, 'workflow endpoint grant operation'),
    ),
    created_at: expectString(object.created_at, 'workflow endpoint grant created_at'),
    updated_at: expectString(object.updated_at, 'workflow endpoint grant updated_at'),
  };
}

function expectPresent(value: unknown, label: string): unknown {
  if (value === undefined || value === null) throw invalid(`${label} must be present.`);
  return value;
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalid(`${label} must be an object.`);
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) throw invalid(`${label} must be an array.`);
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string') throw invalid(`${label} must be a string.`);
  return value;
}

function expectNumber(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) throw invalid(`${label} must be a finite number.`);
  return value;
}

function expectEnum<T extends string>(value: unknown, allowed: readonly T[], label: string): T {
  const candidate = expectString(value, label);
  if (!allowed.includes(candidate as T)) throw invalid(`${label} must be one of ${allowed.join(', ')}.`);
  return candidate as T;
}

function invalid(message: string): WorkflowEndpointCatalogError {
  return new WorkflowEndpointCatalogError(message, 'invalid_payload');
}
