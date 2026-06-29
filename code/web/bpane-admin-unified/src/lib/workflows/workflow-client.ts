import type {
  CreateWorkflowDefinitionRequest,
  CreateWorkflowDefinitionVersionRequest,
  WorkflowDefinitionListResponse,
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionListResponse,
  WorkflowDefinitionVersionResource,
  WorkflowSourceResource,
} from './workflow-types';

export type AccessTokenProvider = () => Promise<string | null> | string | null;
export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export type WorkflowCatalogErrorCode = 'missing_token' | 'http_error' | 'invalid_payload';

export type WorkflowCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class WorkflowCatalogError extends Error {
  readonly status: number | null;
  readonly code: WorkflowCatalogErrorCode;

  constructor(message: string, code: WorkflowCatalogErrorCode, status: number | null = null) {
    super(message);
    this.name = 'WorkflowCatalogError';
    this.code = code;
    this.status = status;
  }
}

export class WorkflowCatalogClient {
  readonly #baseUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider;
  readonly #fetchImpl: FetchLike;
  readonly #onAuthenticationFailure: (() => void) | undefined;

  constructor(options: WorkflowCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
    this.#fetchImpl = options.fetchImpl ?? fetch;
    this.#onAuthenticationFailure = options.onAuthenticationFailure;
  }

  async listDefinitions(): Promise<WorkflowDefinitionListResponse> {
    const response = await this.#request(new URL('/api/v1/workflows', this.#baseUrl), {
      method: 'GET',
      headers: { accept: 'application/json' },
    });
    return toWorkflowDefinitionListResponse(await response.json());
  }

  async createDefinition(request: CreateWorkflowDefinitionRequest): Promise<WorkflowDefinitionResource> {
    const response = await this.#request(new URL('/api/v1/workflows', this.#baseUrl), {
      method: 'POST',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
      },
      body: JSON.stringify(request),
    });
    return toWorkflowDefinitionResource(await response.json());
  }

  async getDefinition(workflowId: string): Promise<WorkflowDefinitionResource> {
    const response = await this.#request(
      new URL(`/api/v1/workflows/${encodeURIComponent(workflowId)}`, this.#baseUrl),
      {
        method: 'GET',
        headers: { accept: 'application/json' },
      },
    );
    return toWorkflowDefinitionResource(await response.json());
  }

  async listDefinitionVersions(workflowId: string): Promise<WorkflowDefinitionVersionListResponse> {
    const response = await this.#request(
      new URL(`/api/v1/workflows/${encodeURIComponent(workflowId)}/versions`, this.#baseUrl),
      {
        method: 'GET',
        headers: { accept: 'application/json' },
      },
    );
    return toWorkflowDefinitionVersionListResponse(await response.json());
  }

  async createDefinitionVersion(
    workflowId: string,
    request: CreateWorkflowDefinitionVersionRequest,
  ): Promise<WorkflowDefinitionVersionResource> {
    const response = await this.#request(
      new URL(`/api/v1/workflows/${encodeURIComponent(workflowId)}/versions`, this.#baseUrl),
      {
        method: 'POST',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        body: JSON.stringify(request),
      },
    );
    return toWorkflowDefinitionVersionResource(await response.json());
  }

  async getDefinitionVersion(workflowId: string, version: string): Promise<WorkflowDefinitionVersionResource> {
    const response = await this.#request(
      new URL(
        `/api/v1/workflows/${encodeURIComponent(workflowId)}/versions/${encodeURIComponent(version)}`,
        this.#baseUrl,
      ),
      {
        method: 'GET',
        headers: { accept: 'application/json' },
      },
    );
    return toWorkflowDefinitionVersionResource(await response.json());
  }

  async #request(input: URL, init: RequestInit): Promise<Response> {
    const accessToken = await this.#accessTokenProvider();
    if (!accessToken) {
      throw new WorkflowCatalogError('No active admin access token is available.', 'missing_token');
    }

    const headers = new Headers(init.headers);
    headers.set('authorization', `Bearer ${accessToken}`);

    const response = await this.#fetchImpl(input, { ...init, headers });
    if (response.status === 401) {
      this.#onAuthenticationFailure?.();
    }
    if (!response.ok) {
      throw new WorkflowCatalogError(
        `Workflow catalog request failed with HTTP ${response.status}.`,
        'http_error',
        response.status,
      );
    }

    return response;
  }
}

export function toWorkflowDefinitionListResponse(payload: unknown): WorkflowDefinitionListResponse {
  const object = expectRecord(payload, 'workflow definition list response');
  return {
    workflows: expectArray(object.workflows, 'workflow definition list workflows')
      .map(toWorkflowDefinitionResource),
  };
}

export function toWorkflowDefinitionVersionListResponse(payload: unknown): WorkflowDefinitionVersionListResponse {
  const object = expectRecord(payload, 'workflow definition version list response');
  return {
    versions: expectArray(object.versions, 'workflow definition version list versions')
      .map(toWorkflowDefinitionVersionResource),
  };
}

export function toWorkflowDefinitionResource(value: unknown): WorkflowDefinitionResource {
  const object = expectRecord(value, 'workflow definition');
  return {
    id: expectString(object.id, 'workflow definition id'),
    name: expectString(object.name, 'workflow definition name'),
    description: optionalString(object.description, 'workflow definition description') ?? null,
    labels: toStringRecord(object.labels ?? {}, 'workflow definition labels'),
    latest_version: optionalString(object.latest_version, 'workflow definition latest_version') ?? null,
    created_at: expectString(object.created_at, 'workflow definition created_at'),
    updated_at: expectString(object.updated_at, 'workflow definition updated_at'),
  };
}

export function toWorkflowDefinitionVersionResource(value: unknown): WorkflowDefinitionVersionResource {
  const object = expectRecord(value, 'workflow definition version');
  const source = object.source === undefined ? undefined : toWorkflowSource(object.source);
  return {
    id: expectString(object.id, 'workflow definition version id'),
    workflow_definition_id: expectString(
      object.workflow_definition_id,
      'workflow definition version workflow_definition_id',
    ),
    version: expectString(object.version, 'workflow definition version version'),
    executor: expectString(object.executor, 'workflow definition version executor'),
    entrypoint: expectString(object.entrypoint, 'workflow definition version entrypoint'),
    ...(source !== undefined ? { source } : {}),
    input_schema: object.input_schema,
    output_schema: object.output_schema,
    default_session: object.default_session,
    allowed_credential_binding_ids: expectStringArray(
      object.allowed_credential_binding_ids ?? [],
      'workflow definition version allowed_credential_binding_ids',
    ),
    allowed_extension_ids: expectStringArray(
      object.allowed_extension_ids ?? [],
      'workflow definition version allowed_extension_ids',
    ),
    allowed_file_workspace_ids: expectStringArray(
      object.allowed_file_workspace_ids ?? [],
      'workflow definition version allowed_file_workspace_ids',
    ),
    created_at: expectString(object.created_at, 'workflow definition version created_at'),
  };
}

function toWorkflowSource(value: unknown): WorkflowSourceResource | null {
  if (value === null) {
    return null;
  }
  const object = expectRecord(value, 'workflow definition version source');
  const kind = expectString(object.kind, 'workflow definition version source kind');
  if (kind !== 'git') {
    throw new WorkflowCatalogError(
      `workflow definition version source kind ${kind} is not supported.`,
      'invalid_payload',
    );
  }
  return {
    kind,
    repository_url: expectString(object.repository_url, 'workflow definition version source repository_url'),
    ref: optionalString(object.ref, 'workflow definition version source ref') ?? null,
    resolved_commit: optionalString(
      object.resolved_commit,
      'workflow definition version source resolved_commit',
    ) ?? null,
    root_path: optionalString(object.root_path, 'workflow definition version source root_path') ?? null,
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new WorkflowCatalogError(`${label} must be an object.`, 'invalid_payload');
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new WorkflowCatalogError(`${label} must be an array.`, 'invalid_payload');
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string') {
    throw new WorkflowCatalogError(`${label} must be a string.`, 'invalid_payload');
  }
  return value;
}

function optionalString(value: unknown, label: string): string | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return expectString(value, label);
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
