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
  CreateSessionFileBindingRequest,
  SessionFileBindingListResponse,
  SessionFileBindingMode,
  SessionFileBindingResource,
  SessionFileBindingState,
  SessionFileListResponse,
  SessionFileResource,
  SessionFileSource,
} from './session-file-types';

const FILE_SOURCES = ['browser_upload', 'browser_download'] satisfies readonly SessionFileSource[];
const BINDING_MODES = ['read_only', 'read_write', 'scratch_output'] satisfies readonly SessionFileBindingMode[];
const BINDING_STATES = ['pending', 'materialized', 'failed', 'removed'] satisfies readonly SessionFileBindingState[];

export type SessionFileClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class SessionFileClientError extends AdminApiRequestError {
  constructor(
    message: string,
    code: AdminApiRequestErrorCode,
    status: number | null = null,
    failure?: AdminApiRequestFailure,
  ) {
    super(message, failure ?? { code, status, message });
    this.name = 'SessionFileClientError';
  }
}

export class SessionFileClient {
  readonly #baseUrl: URL;
  readonly #api: AuthenticatedApiClient;

  constructor(options: SessionFileClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#api = new AuthenticatedApiClient({
      baseUrl: this.#baseUrl,
      accessTokenProvider: options.accessTokenProvider,
      ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
      ...(options.onAuthenticationFailure === undefined
        ? {}
        : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) => new SessionFileClientError(
        formatAdminApiRequestError('Session file request', failure),
        failure.code,
        failure.status,
        failure,
      ),
    });
  }

  async listSessionFiles(sessionId: string): Promise<SessionFileListResponse> {
    const response = await this.#request(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/files`,
      { method: 'GET', headers: { accept: 'application/json' } },
    );
    return toSessionFileListResponse(await response.json());
  }

  async downloadSessionFile(file: SessionFileResource): Promise<Blob> {
    const response = await this.#request(file.content_path, {
      method: 'GET',
      headers: { accept: file.media_type ?? 'application/octet-stream' },
    });
    return await response.blob();
  }

  async listSessionFileBindings(sessionId: string): Promise<SessionFileBindingListResponse> {
    const response = await this.#request(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/file-bindings`,
      { method: 'GET', headers: { accept: 'application/json' } },
    );
    return toSessionFileBindingListResponse(await response.json());
  }

  async createSessionFileBinding(
    sessionId: string,
    request: CreateSessionFileBindingRequest,
  ): Promise<SessionFileBindingResource> {
    const response = await this.#request(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/file-bindings`,
      {
        method: 'POST',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        body: JSON.stringify(request),
      },
    );
    return toSessionFileBindingResource(await response.json());
  }

  async removeSessionFileBinding(sessionId: string, bindingId: string): Promise<SessionFileBindingResource> {
    const response = await this.#request(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/file-bindings/${encodeURIComponent(bindingId)}`,
      { method: 'DELETE', headers: { accept: 'application/json' } },
    );
    return toSessionFileBindingResource(await response.json());
  }

  async downloadSessionFileBinding(binding: SessionFileBindingResource): Promise<Blob> {
    const response = await this.#request(binding.content_path, {
      method: 'GET',
      headers: { accept: binding.media_type ?? 'application/octet-stream' },
    });
    return await response.blob();
  }

  async #request(path: string, init: RequestInit): Promise<Response> {
    return await this.#api.request(new URL(path, this.#baseUrl), init);
  }
}

export function toSessionFileListResponse(payload: unknown): SessionFileListResponse {
  const object = expectRecord(payload, 'session file list response');
  return {
    files: expectArray(object.files, 'session file list files').map(toSessionFileResource),
  };
}

export function toSessionFileResource(payload: unknown): SessionFileResource {
  const object = expectRecord(payload, 'session file');
  return {
    id: expectString(object.id, 'session file id'),
    session_id: expectString(object.session_id, 'session file session_id'),
    name: expectString(object.name, 'session file name'),
    media_type: optionalString(object.media_type, 'session file media_type') ?? null,
    byte_count: expectNonNegativeNumber(object.byte_count, 'session file byte_count'),
    sha256_hex: expectString(object.sha256_hex, 'session file sha256_hex'),
    source: expectEnum(object.source, FILE_SOURCES, 'session file source'),
    labels: expectStringRecord(object.labels, 'session file labels'),
    content_path: expectString(object.content_path, 'session file content_path'),
    created_at: expectString(object.created_at, 'session file created_at'),
    updated_at: expectString(object.updated_at, 'session file updated_at'),
  };
}

export function toSessionFileBindingListResponse(payload: unknown): SessionFileBindingListResponse {
  const object = expectRecord(payload, 'session file binding list response');
  return {
    bindings: expectArray(object.bindings, 'session file binding list bindings')
      .map(toSessionFileBindingResource),
  };
}

export function toSessionFileBindingResource(payload: unknown): SessionFileBindingResource {
  const object = expectRecord(payload, 'session file binding');
  return {
    id: expectString(object.id, 'session file binding id'),
    session_id: expectString(object.session_id, 'session file binding session_id'),
    workspace_id: expectString(object.workspace_id, 'session file binding workspace_id'),
    file_id: expectString(object.file_id, 'session file binding file_id'),
    file_name: expectString(object.file_name, 'session file binding file_name'),
    media_type: optionalString(object.media_type, 'session file binding media_type') ?? null,
    byte_count: expectNonNegativeNumber(object.byte_count, 'session file binding byte_count'),
    sha256_hex: expectString(object.sha256_hex, 'session file binding sha256_hex'),
    provenance: optionalRecord(object.provenance, 'session file binding provenance'),
    mount_path: expectString(object.mount_path, 'session file binding mount_path'),
    mode: expectEnum(object.mode, BINDING_MODES, 'session file binding mode'),
    state: expectEnum(object.state, BINDING_STATES, 'session file binding state'),
    error: optionalString(object.error, 'session file binding error') ?? null,
    labels: expectStringRecord(object.labels, 'session file binding labels'),
    content_path: expectString(object.content_path, 'session file binding content_path'),
    created_at: expectString(object.created_at, 'session file binding created_at'),
    updated_at: expectString(object.updated_at, 'session file binding updated_at'),
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw invalidPayload(`${label} must be an object.`);
  }
  return value as Record<string, unknown>;
}

function optionalRecord(value: unknown, label: string): Readonly<Record<string, unknown>> | null {
  if (value === undefined || value === null) {
    return null;
  }
  return expectRecord(value, label);
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw invalidPayload(`${label} must be an array.`);
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw invalidPayload(`${label} must be a non-empty string.`);
  }
  return value;
}

function optionalString(value: unknown, label: string): string | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  if (typeof value !== 'string') {
    throw invalidPayload(`${label} must be a string.`);
  }
  return value;
}

function expectNonNegativeNumber(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) {
    throw invalidPayload(`${label} must be a non-negative number.`);
  }
  return value;
}

function expectStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
  const object = expectRecord(value, label);
  for (const [key, entry] of Object.entries(object)) {
    if (typeof entry !== 'string') {
      throw invalidPayload(`${label}.${key} must be a string.`);
    }
  }
  return object as Readonly<Record<string, string>>;
}

function expectEnum<const T extends string>(value: unknown, values: readonly T[], label: string): T {
  if (typeof value !== 'string' || !values.includes(value as T)) {
    throw invalidPayload(`${label} is unsupported.`);
  }
  return value as T;
}

function invalidPayload(message: string): SessionFileClientError {
  return new SessionFileClientError(message, 'invalid_payload');
}
