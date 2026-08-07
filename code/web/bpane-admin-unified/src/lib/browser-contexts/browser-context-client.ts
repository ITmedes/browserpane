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
  BrowserContextListResponse,
  BrowserContextExportArchive,
  BrowserContextPersistenceMode,
  BrowserContextProjectOptionsResponse,
  BrowserContextProjectResource,
  BrowserContextResource,
  BrowserContextState,
  BrowserContextUsageResource,
  CloneBrowserContextRequest,
  CreateBrowserContextRequest,
  ImportBrowserContextRequest,
} from './browser-context-types';

const BROWSER_CONTEXT_STATES = ['ready', 'deleted'] satisfies readonly BrowserContextState[];
const BROWSER_CONTEXT_PERSISTENCE_MODES = ['reusable', 'ephemeral'] satisfies readonly BrowserContextPersistenceMode[];
const PROJECT_STATES = ['active', 'archived'] satisfies readonly BrowserContextProjectResource['state'][];

export type { AccessTokenProvider, FetchLike } from '$lib/api/authenticated-api';
export type BrowserContextCatalogErrorCode = AdminApiRequestErrorCode;

export type BrowserContextCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class BrowserContextCatalogError extends AdminApiRequestError {
  constructor(
    message: string,
    code: BrowserContextCatalogErrorCode,
    status: number | null = null,
    failure?: AdminApiRequestFailure,
  ) {
    super(message, failure ?? { code, status, message });
    this.name = 'BrowserContextCatalogError';
  }
}

export class BrowserContextCatalogClient {
  readonly #baseUrl: URL;
  readonly #api: AuthenticatedApiClient;

  constructor(options: BrowserContextCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#api = new AuthenticatedApiClient({
      baseUrl: this.#baseUrl,
      accessTokenProvider: options.accessTokenProvider,
      ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
      ...(options.onAuthenticationFailure === undefined
        ? {}
        : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) => new BrowserContextCatalogError(
        formatAdminApiRequestError('Browser context catalog request', failure),
        failure.code,
        failure.status,
        failure,
      ),
    });
  }

  async listBrowserContexts(): Promise<BrowserContextListResponse> {
    const response = await this.#request(new URL('/api/v1/browser-contexts', this.#baseUrl), {
      method: 'GET',
      headers: {
        accept: 'application/json',
      },
    });

    return toBrowserContextListResponse(await response.json());
  }

  async getBrowserContext(contextId: string): Promise<BrowserContextResource> {
    const response = await this.#request(
      new URL(`/api/v1/browser-contexts/${encodeURIComponent(contextId)}`, this.#baseUrl),
      {
        method: 'GET',
        headers: {
          accept: 'application/json',
        },
      },
    );

    return toBrowserContextResource(await response.json());
  }

  async createBrowserContext(request: CreateBrowserContextRequest): Promise<BrowserContextResource> {
    const response = await this.#request(new URL('/api/v1/browser-contexts', this.#baseUrl), {
      method: 'POST',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    return toBrowserContextResource(await response.json());
  }

  async deleteBrowserContext(contextId: string): Promise<BrowserContextResource> {
    const response = await this.#request(
      new URL(`/api/v1/browser-contexts/${encodeURIComponent(contextId)}`, this.#baseUrl),
      {
        method: 'DELETE',
        headers: {
          accept: 'application/json',
        },
      },
    );

    return toBrowserContextResource(await response.json());
  }

  async cloneBrowserContext(
    contextId: string,
    request: CloneBrowserContextRequest,
  ): Promise<BrowserContextResource> {
    const response = await this.#request(
      new URL(`/api/v1/browser-contexts/${encodeURIComponent(contextId)}/clone`, this.#baseUrl),
      {
        method: 'POST',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        body: JSON.stringify(request),
      },
    );

    return toBrowserContextResource(await response.json());
  }

  async exportBrowserContext(contextId: string): Promise<BrowserContextExportArchive> {
    const response = await this.#request(
      new URL(`/api/v1/browser-contexts/${encodeURIComponent(contextId)}/export`, this.#baseUrl),
      {
        method: 'GET',
        headers: {
          accept: 'application/zip',
        },
      },
    );

    return {
      blob: await response.blob(),
      filename: browserContextExportFilename(response.headers, contextId),
    };
  }

  async importBrowserContext(request: ImportBrowserContextRequest): Promise<BrowserContextResource> {
    const headers = browserContextImportHeaders(request);
    const response = await this.#request(new URL('/api/v1/browser-contexts/import', this.#baseUrl), {
      method: 'POST',
      headers,
      body: request.archive,
    });

    return toBrowserContextResource(await response.json());
  }

  async listProjectOptions(): Promise<BrowserContextProjectOptionsResponse> {
    const response = await this.#request(new URL('/api/v1/projects', this.#baseUrl), {
      method: 'GET',
      headers: {
        accept: 'application/json',
      },
    });

    return toBrowserContextProjectOptionsResponse(await response.json());
  }

  async #request(input: URL, init: RequestInit): Promise<Response> {
    return await this.#api.request(input, init);
  }
}

export function browserContextExportFilename(headers: Headers, contextId: string): string {
  const disposition = headers.get('content-disposition') ?? '';
  const encodedMatch = disposition.match(/filename\*\s*=\s*UTF-8''([^;]+)/i);
  const quotedMatch = disposition.match(/filename\s*=\s*"([^"]*)"/i);
  const plainMatch = disposition.match(/filename\s*=\s*([^;]+)/i);
  let candidate = encodedMatch?.[1] ?? quotedMatch?.[1] ?? plainMatch?.[1] ?? '';
  if (encodedMatch?.[1]) {
    try {
      candidate = decodeURIComponent(encodedMatch[1]);
    } catch {
      candidate = '';
    }
  }
  return sanitizeBrowserContextArchiveFilename(candidate, contextId);
}

export function sanitizeBrowserContextArchiveFilename(candidate: string, contextId: string): string {
  const normalized = candidate
    .normalize('NFKC')
    .replace(/[\\/]/g, '-')
    .replace(/[^a-zA-Z0-9._-]+/g, '-')
    .replace(/^[.-]+|[.-]+$/g, '')
    .slice(0, 120);
  const fallbackId = contextId
    .replace(/[^a-zA-Z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80) || 'context';
  const filename = normalized || `browserpane-browser-context-${fallbackId}`;
  return filename.toLowerCase().endsWith('.zip') ? filename : `${filename}.zip`;
}

function browserContextImportHeaders(request: ImportBrowserContextRequest): Headers {
  const headers = new Headers({
    accept: 'application/json',
    'content-type': 'application/zip',
    'x-bpane-browser-context-name': request.name,
  });
  setOptionalHeader(headers, 'x-bpane-browser-context-project-id', request.project_id);
  setOptionalHeader(headers, 'x-bpane-browser-context-description', request.description);
  if (request.labels !== undefined) {
    headers.set('x-bpane-browser-context-labels', JSON.stringify(request.labels));
  }
  setOptionalHeader(headers, 'x-bpane-browser-context-retention-sec', request.retention_sec);
  setOptionalHeader(
    headers,
    'x-bpane-browser-context-max-profile-storage-bytes',
    request.max_profile_storage_bytes,
  );
  return headers;
}

function setOptionalHeader(headers: Headers, name: string, value: string | number | null | undefined): void {
  if (value !== undefined && value !== null) {
    headers.set(name, String(value));
  }
}

export function toBrowserContextListResponse(payload: unknown): BrowserContextListResponse {
  const object = expectRecord(payload, 'browser context list response');
  return {
    contexts: expectArray(object.contexts, 'browser context list contexts').map(toBrowserContextResource),
  };
}

export function toBrowserContextProjectOptionsResponse(payload: unknown): BrowserContextProjectOptionsResponse {
  const object = expectRecord(payload, 'project list response');
  return {
    projects: expectArray(object.projects, 'project list projects').map(toBrowserContextProjectResourceFromRequired),
  };
}

export function toBrowserContextResource(value: unknown): BrowserContextResource {
  const object = expectRecord(value, 'browser context');
  return {
    id: expectString(object.id, 'browser context id'),
    project_id: optionalString(object.project_id, 'browser context project_id') ?? null,
    project: toBrowserContextProjectResource(object.project) ?? null,
    name: expectString(object.name, 'browser context name'),
    description: optionalString(object.description, 'browser context description') ?? null,
    labels: toStringRecord(object.labels ?? {}, 'browser context labels'),
    persistence_mode: expectEnum(
      object.persistence_mode,
      BROWSER_CONTEXT_PERSISTENCE_MODES,
      'browser context persistence_mode',
    ),
    retention_sec: optionalNumber(object.retention_sec, 'browser context retention_sec') ?? null,
    retention_expires_at: optionalString(object.retention_expires_at, 'browser context retention_expires_at') ?? null,
    max_profile_storage_bytes:
      optionalNumber(object.max_profile_storage_bytes, 'browser context max_profile_storage_bytes') ?? null,
    state: expectEnum(object.state, BROWSER_CONTEXT_STATES, 'browser context state'),
    usage: toBrowserContextUsageResource(object.usage),
    created_at: expectString(object.created_at, 'browser context created_at'),
    updated_at: expectString(object.updated_at, 'browser context updated_at'),
    last_used_at: optionalString(object.last_used_at, 'browser context last_used_at') ?? null,
    deleted_at: optionalString(object.deleted_at, 'browser context deleted_at') ?? null,
  };
}

function toBrowserContextProjectResource(value: unknown): BrowserContextProjectResource | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return toBrowserContextProjectResourceFromRequired(value);
}

function toBrowserContextProjectResourceFromRequired(value: unknown): BrowserContextProjectResource {
  const object = expectRecord(value, 'browser context project');
  return {
    id: expectString(object.id, 'browser context project id'),
    name: expectString(object.name, 'browser context project name'),
    state: expectEnum(object.state, PROJECT_STATES, 'browser context project state'),
  };
}

function toBrowserContextUsageResource(value: unknown): BrowserContextUsageResource {
  const object = value === undefined || value === null
    ? {}
    : expectRecord(value, 'browser context usage');
  return {
    visible_session_count: expectNumber(object.visible_session_count ?? 0, 'browser context visible_session_count'),
    active_runtime_session_count: expectNumber(
      object.active_runtime_session_count ?? 0,
      'browser context active_runtime_session_count',
    ),
    active_runtime_session_id:
      optionalString(object.active_runtime_session_id, 'browser context active_runtime_session_id') ?? null,
    profile_storage_bytes:
      optionalNumber(object.profile_storage_bytes, 'browser context profile_storage_bytes') ?? null,
    profile_storage_limit_exceeded:
      expectBoolean(object.profile_storage_limit_exceeded ?? false, 'browser context profile_storage_limit_exceeded'),
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new BrowserContextCatalogError(`${label} must be an object.`, 'invalid_payload');
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new BrowserContextCatalogError(`${label} must be an array.`, 'invalid_payload');
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string') {
    throw new BrowserContextCatalogError(`${label} must be a string.`, 'invalid_payload');
  }
  return value;
}

function optionalString(value: unknown, label: string): string | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return expectString(value, label);
}

function expectNumber(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new BrowserContextCatalogError(`${label} must be a finite number.`, 'invalid_payload');
  }
  return value;
}

function optionalNumber(value: unknown, label: string): number | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return expectNumber(value, label);
}

function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') {
    throw new BrowserContextCatalogError(`${label} must be a boolean.`, 'invalid_payload');
  }
  return value;
}

function expectEnum<T extends string>(value: unknown, allowed: readonly T[], label: string): T {
  const stringValue = expectString(value, label);
  if (!allowed.includes(stringValue as T)) {
    throw new BrowserContextCatalogError(
      `${label} must be one of ${allowed.join(', ')}.`,
      'invalid_payload',
    );
  }
  return stringValue as T;
}

function toStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
  const object = expectRecord(value, label);
  return Object.fromEntries(
    Object.entries(object).map(([key, item]) => [key, expectString(item, `${label}.${key}`)]),
  );
}
