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
  CreateCredentialBindingRequest,
  CredentialBindingListResponse,
  CredentialBindingProjectOptionsResponse,
  CredentialBindingProjectResource,
  CredentialBindingResource,
  CredentialInjectionMode,
  CredentialTotpMetadata,
} from './credential-binding-types';

const INJECTION_MODES = ['form_fill', 'cookie_seed', 'storage_seed', 'totp_fill'] satisfies readonly CredentialInjectionMode[];
const PROJECT_STATES = ['active', 'archived'] satisfies readonly CredentialBindingProjectResource['state'][];

export type CredentialBindingCatalogErrorCode = AdminApiRequestErrorCode;

export class CredentialBindingCatalogError extends AdminApiRequestError {
  constructor(
    message: string,
    code: CredentialBindingCatalogErrorCode,
    status: number | null = null,
    failure?: AdminApiRequestFailure,
  ) {
    super(message, failure ?? { code, status, message });
    this.name = 'CredentialBindingCatalogError';
  }
}

export class CredentialBindingCatalogClient {
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
      ...(options.onAuthenticationFailure === undefined ? {} : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) => new CredentialBindingCatalogError(
        formatAdminApiRequestError('Credential binding catalog request', failure),
        failure.code,
        failure.status,
        failure,
      ),
    });
  }

  async listCredentialBindings(): Promise<CredentialBindingListResponse> {
    const response = await this.#request('/api/v1/credential-bindings', { method: 'GET' });
    return toCredentialBindingListResponse(await response.json());
  }

  async createCredentialBinding(request: CreateCredentialBindingRequest): Promise<CredentialBindingResource> {
    const response = await this.#request('/api/v1/credential-bindings', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(request),
    });
    return toCredentialBindingResource(await response.json());
  }

  async getCredentialBinding(bindingId: string): Promise<CredentialBindingResource> {
    const response = await this.#request(`/api/v1/credential-bindings/${encodeURIComponent(bindingId)}`, { method: 'GET' });
    return toCredentialBindingResource(await response.json());
  }

  async listProjectOptions(): Promise<CredentialBindingProjectOptionsResponse> {
    const response = await this.#request('/api/v1/projects', { method: 'GET' });
    const payload = expectRecord(await response.json(), 'project list response');
    return {
      projects: expectArray(payload.projects, 'project list projects').map(toProjectResource),
    };
  }

  async #request(path: string, init: RequestInit): Promise<Response> {
    const headers = new Headers(init.headers);
    headers.set('accept', 'application/json');
    return await this.#api.request(new URL(path, this.#baseUrl), { ...init, headers });
  }
}

export function toCredentialBindingListResponse(payload: unknown): CredentialBindingListResponse {
  const object = expectRecord(payload, 'credential binding list response');
  return {
    credential_bindings: expectArray(object.credential_bindings, 'credential binding list resources')
      .map(toCredentialBindingResource),
  };
}

export function toCredentialBindingResource(value: unknown): CredentialBindingResource {
  const object = expectRecord(value, 'credential binding');
  return {
    id: expectString(object.id, 'credential binding id'),
    project_id: optionalString(object.project_id, 'credential binding project_id') ?? null,
    project: nullableProjectResource(object.project),
    name: expectString(object.name, 'credential binding name'),
    provider: expectEnum(object.provider, ['vault_kv_v2'] as const, 'credential binding provider'),
    external_ref: expectString(object.external_ref, 'credential binding external_ref'),
    namespace: optionalString(object.namespace, 'credential binding namespace') ?? null,
    allowed_origins: expectArray(object.allowed_origins, 'credential binding allowed_origins')
      .map((origin) => expectString(origin, 'credential binding allowed origin')),
    injection_mode: expectEnum(object.injection_mode, INJECTION_MODES, 'credential binding injection_mode'),
    totp: nullableTotp(object.totp),
    labels: toStringRecord(object.labels, 'credential binding labels'),
    created_at: expectString(object.created_at, 'credential binding created_at'),
    updated_at: expectString(object.updated_at, 'credential binding updated_at'),
  };
}

function nullableProjectResource(value: unknown): CredentialBindingProjectResource | null {
  return value === undefined || value === null ? null : toProjectResource(value);
}

function toProjectResource(value: unknown): CredentialBindingProjectResource {
  const object = expectRecord(value, 'credential binding project');
  return {
    id: expectString(object.id, 'credential binding project id'),
    name: expectString(object.name, 'credential binding project name'),
    state: expectEnum(object.state, PROJECT_STATES, 'credential binding project state'),
  };
}

function nullableTotp(value: unknown): CredentialTotpMetadata | null {
  if (value === undefined || value === null) return null;
  const object = expectRecord(value, 'credential binding totp');
  return {
    issuer: optionalString(object.issuer, 'credential binding totp issuer') ?? null,
    account_name: optionalString(object.account_name, 'credential binding totp account_name') ?? null,
    period_sec: optionalNumber(object.period_sec, 'credential binding totp period_sec') ?? null,
    digits: optionalNumber(object.digits, 'credential binding totp digits') ?? null,
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new CredentialBindingCatalogError(`${label} must be an object.`, 'invalid_payload');
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) throw new CredentialBindingCatalogError(`${label} must be an array.`, 'invalid_payload');
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string') throw new CredentialBindingCatalogError(`${label} must be a string.`, 'invalid_payload');
  return value;
}

function optionalString(value: unknown, label: string): string | null | undefined {
  return value === undefined || value === null ? value : expectString(value, label);
}

function optionalNumber(value: unknown, label: string): number | null | undefined {
  if (value === undefined || value === null) return value;
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new CredentialBindingCatalogError(`${label} must be a finite number.`, 'invalid_payload');
  }
  return value;
}

function expectEnum<T extends string>(value: unknown, allowed: readonly T[], label: string): T {
  const candidate = expectString(value, label);
  if (!allowed.includes(candidate as T)) {
    throw new CredentialBindingCatalogError(`${label} must be one of ${allowed.join(', ')}.`, 'invalid_payload');
  }
  return candidate as T;
}

function toStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
  const object = expectRecord(value, label);
  return Object.fromEntries(Object.entries(object).map(([key, item]) => [key, expectString(item, `${label}.${key}`)]));
}
