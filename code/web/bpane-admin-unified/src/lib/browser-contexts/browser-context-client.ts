import type {
  BrowserContextListResponse,
  BrowserContextPersistenceMode,
  BrowserContextProjectOptionsResponse,
  BrowserContextProjectResource,
  BrowserContextResource,
  BrowserContextState,
  BrowserContextUsageResource,
  CreateBrowserContextRequest,
} from './browser-context-types';

const BROWSER_CONTEXT_STATES = ['ready', 'deleted'] satisfies readonly BrowserContextState[];
const BROWSER_CONTEXT_PERSISTENCE_MODES = ['reusable', 'ephemeral'] satisfies readonly BrowserContextPersistenceMode[];
const PROJECT_STATES = ['active', 'archived'] satisfies readonly BrowserContextProjectResource['state'][];

export type AccessTokenProvider = () => Promise<string | null> | string | null;
export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export type BrowserContextCatalogErrorCode = 'missing_token' | 'http_error' | 'invalid_payload';

export type BrowserContextCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class BrowserContextCatalogError extends Error {
  readonly status: number | null;
  readonly code: BrowserContextCatalogErrorCode;

  constructor(message: string, code: BrowserContextCatalogErrorCode, status: number | null = null) {
    super(message);
    this.name = 'BrowserContextCatalogError';
    this.code = code;
    this.status = status;
  }
}

export class BrowserContextCatalogClient {
  readonly #baseUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider;
  readonly #fetchImpl: FetchLike;
  readonly #onAuthenticationFailure: (() => void) | undefined;

  constructor(options: BrowserContextCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
    this.#fetchImpl = options.fetchImpl ?? fetch;
    this.#onAuthenticationFailure = options.onAuthenticationFailure;
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
    const accessToken = await this.#accessTokenProvider();
    if (!accessToken) {
      throw new BrowserContextCatalogError('No active admin access token is available.', 'missing_token');
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
      throw new BrowserContextCatalogError(
        `Browser context catalog request failed with HTTP ${response.status}.`,
        'http_error',
        response.status,
      );
    }

    return response;
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
