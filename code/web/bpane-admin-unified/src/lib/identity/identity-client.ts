import {
  AdminApiRequestError,
  AuthenticatedApiClient,
  formatAdminApiRequestError,
  type AccessTokenProvider,
  type AdminApiRequestErrorCode,
  type AdminApiRequestFailure,
  type FetchLike,
} from '$lib/api/authenticated-api';
import { toProjectResource } from '$lib/projects/project-client';
import type {
  IdentityAccessReviewResponse,
  IdentityDelegatedPrincipalResource,
  IdentityMappingKind,
  IdentityMappingListResponse,
  IdentityMappingResource,
  IdentityMappingReviewResource,
  IdentityMappingState,
  IdentityPrincipalResource,
  IdentityPrincipalType,
  IdentityResourceCounts,
  IdentityServicePrincipalReviewResource,
  IdentityUnmappedPrincipalSignalResource,
  ServicePrincipalListResponse,
  ServicePrincipalResource,
  ServicePrincipalState,
  UpsertIdentityMappingRequest,
  UpsertServicePrincipalRequest,
} from './identity-types';

const SERVICE_PRINCIPAL_STATES = ['active', 'disabled'] satisfies readonly ServicePrincipalState[];
const IDENTITY_MAPPING_KINDS = ['user', 'group', 'claim', 'service_principal'] satisfies readonly IdentityMappingKind[];
const IDENTITY_MAPPING_STATES = ['active', 'disabled'] satisfies readonly IdentityMappingState[];
const IDENTITY_PRINCIPAL_TYPES = ['user', 'service_principal', 'legacy_dev_token'] satisfies readonly IdentityPrincipalType[];

export type { AccessTokenProvider, FetchLike } from '$lib/api/authenticated-api';
export type IdentityCatalogErrorCode = AdminApiRequestErrorCode;

export type IdentityCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class IdentityCatalogError extends AdminApiRequestError {
  constructor(
    message: string,
    code: IdentityCatalogErrorCode,
    status: number | null = null,
    failure?: AdminApiRequestFailure,
  ) {
    super(message, failure ?? { code, status, message });
    this.name = 'IdentityCatalogError';
  }
}

export class IdentityCatalogClient {
  readonly #baseUrl: URL;
  readonly #api: AuthenticatedApiClient;

  constructor(options: IdentityCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#api = new AuthenticatedApiClient({
      baseUrl: this.#baseUrl,
      accessTokenProvider: options.accessTokenProvider,
      ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
      ...(options.onAuthenticationFailure === undefined
        ? {}
        : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) => new IdentityCatalogError(
        formatAdminApiRequestError('Identity request', failure),
        failure.code,
        failure.status,
        failure,
      ),
    });
  }

  async getCurrentPrincipal(): Promise<IdentityPrincipalResource> {
    const response = await this.#request('/api/v1/identity/me', { method: 'GET' });
    return toIdentityPrincipalResource(await response.json());
  }

  async getAccessReview(): Promise<IdentityAccessReviewResponse> {
    const response = await this.#request('/api/v1/identity/access-review', { method: 'GET' });
    return toIdentityAccessReviewResponse(await response.json());
  }

  async listServicePrincipals(): Promise<ServicePrincipalListResponse> {
    const response = await this.#request('/api/v1/service-principals', { method: 'GET' });
    return toServicePrincipalListResponse(await response.json());
  }

  async getServicePrincipal(servicePrincipalId: string): Promise<ServicePrincipalResource> {
    const response = await this.#request(`/api/v1/service-principals/${encodeURIComponent(servicePrincipalId)}`, {
      method: 'GET',
    });
    return toServicePrincipalResource(await response.json());
  }

  async createServicePrincipal(request: UpsertServicePrincipalRequest): Promise<ServicePrincipalResource> {
    const response = await this.#request('/api/v1/service-principals', jsonRequest('POST', request));
    return toServicePrincipalResource(await response.json());
  }

  async updateServicePrincipal(
    servicePrincipalId: string,
    request: UpsertServicePrincipalRequest,
  ): Promise<ServicePrincipalResource> {
    const response = await this.#request(
      `/api/v1/service-principals/${encodeURIComponent(servicePrincipalId)}`,
      jsonRequest('PUT', request),
    );
    return toServicePrincipalResource(await response.json());
  }

  async listIdentityMappings(): Promise<IdentityMappingListResponse> {
    const response = await this.#request('/api/v1/identity-mappings', { method: 'GET' });
    return toIdentityMappingListResponse(await response.json());
  }

  async getIdentityMapping(identityMappingId: string): Promise<IdentityMappingResource> {
    const response = await this.#request(`/api/v1/identity-mappings/${encodeURIComponent(identityMappingId)}`, {
      method: 'GET',
    });
    return toIdentityMappingResource(await response.json());
  }

  async createIdentityMapping(request: UpsertIdentityMappingRequest): Promise<IdentityMappingResource> {
    const response = await this.#request('/api/v1/identity-mappings', jsonRequest('POST', request));
    return toIdentityMappingResource(await response.json());
  }

  async updateIdentityMapping(
    identityMappingId: string,
    request: UpsertIdentityMappingRequest,
  ): Promise<IdentityMappingResource> {
    const response = await this.#request(
      `/api/v1/identity-mappings/${encodeURIComponent(identityMappingId)}`,
      jsonRequest('PUT', request),
    );
    return toIdentityMappingResource(await response.json());
  }

  async #request(path: string, init: RequestInit): Promise<Response> {
    return await this.#api.request(new URL(path, this.#baseUrl), {
      ...init,
      headers: {
        accept: 'application/json',
        ...init.headers,
      },
    });
  }
}

export function toIdentityAccessReviewResponse(payload: unknown): IdentityAccessReviewResponse {
  const object = expectRecord(payload, 'identity access review');
  return {
    principal: toIdentityPrincipalResource(object.principal),
    generated_at: expectString(object.generated_at, 'identity access review generated_at'),
    projects: expectArray(object.projects, 'identity access review projects').map(toProjectResource),
    resource_counts: toIdentityResourceCounts(object.resource_counts),
    identity_mappings: expectArray(object.identity_mappings, 'identity access review identity_mappings')
      .map(toIdentityMappingReviewResource),
    unmapped_principal_signals: expectArray(
      object.unmapped_principal_signals,
      'identity access review unmapped_principal_signals',
    ).map(toIdentityUnmappedPrincipalSignalResource),
    service_principals: expectArray(object.service_principals, 'identity access review service_principals')
      .map(toIdentityServicePrincipalReviewResource),
    delegated_principals: expectArray(object.delegated_principals, 'identity access review delegated_principals')
      .map(toIdentityDelegatedPrincipalResource),
  };
}

export function toIdentityPrincipalResource(payload: unknown): IdentityPrincipalResource {
  const object = expectRecord(payload, 'identity principal');
  return {
    subject: expectString(object.subject, 'identity principal subject'),
    issuer: expectString(object.issuer, 'identity principal issuer'),
    display_name: optionalString(object.display_name, 'identity principal display_name') ?? null,
    client_id: optionalString(object.client_id, 'identity principal client_id') ?? null,
    principal_type: expectEnum(object.principal_type, IDENTITY_PRINCIPAL_TYPES, 'identity principal type'),
  };
}

export function toServicePrincipalListResponse(payload: unknown): ServicePrincipalListResponse {
  const object = expectRecord(payload, 'service principal list response');
  return {
    service_principals: expectArray(object.service_principals, 'service principal list service_principals')
      .map(toServicePrincipalResource),
  };
}

export function toServicePrincipalResource(payload: unknown): ServicePrincipalResource {
  const object = expectRecord(payload, 'service principal');
  return {
    id: expectString(object.id, 'service principal id'),
    name: expectString(object.name, 'service principal name'),
    description: optionalString(object.description, 'service principal description') ?? null,
    client_id: expectString(object.client_id, 'service principal client_id'),
    issuer: expectString(object.issuer, 'service principal issuer'),
    labels: toStringRecord(object.labels, 'service principal labels'),
    scopes: toStringArray(object.scopes, 'service principal scopes'),
    allowed_project_ids: toStringArray(object.allowed_project_ids, 'service principal allowed_project_ids'),
    state: expectEnum(object.state, SERVICE_PRINCIPAL_STATES, 'service principal state'),
    last_seen_at: optionalString(object.last_seen_at, 'service principal last_seen_at') ?? null,
    last_delegated_at: optionalString(object.last_delegated_at, 'service principal last_delegated_at') ?? null,
    created_at: expectString(object.created_at, 'service principal created_at'),
    updated_at: expectString(object.updated_at, 'service principal updated_at'),
  };
}

export function toIdentityMappingListResponse(payload: unknown): IdentityMappingListResponse {
  const object = expectRecord(payload, 'identity mapping list response');
  return {
    identity_mappings: expectArray(object.identity_mappings, 'identity mapping list identity_mappings')
      .map(toIdentityMappingResource),
  };
}

export function toIdentityMappingResource(payload: unknown): IdentityMappingResource {
  const object = expectRecord(payload, 'identity mapping');
  return {
    id: expectString(object.id, 'identity mapping id'),
    name: expectString(object.name, 'identity mapping name'),
    description: optionalString(object.description, 'identity mapping description') ?? null,
    kind: expectEnum(object.kind, IDENTITY_MAPPING_KINDS, 'identity mapping kind'),
    issuer: expectString(object.issuer, 'identity mapping issuer'),
    external_id: expectString(object.external_id, 'identity mapping external_id'),
    claim_name: optionalString(object.claim_name, 'identity mapping claim_name') ?? null,
    service_principal_id: optionalString(
      object.service_principal_id,
      'identity mapping service_principal_id',
    ) ?? null,
    project_id: expectString(object.project_id, 'identity mapping project_id'),
    labels: toStringRecord(object.labels, 'identity mapping labels'),
    scopes: toStringArray(object.scopes, 'identity mapping scopes'),
    state: expectEnum(object.state, IDENTITY_MAPPING_STATES, 'identity mapping state'),
    last_seen_at: optionalString(object.last_seen_at, 'identity mapping last_seen_at') ?? null,
    created_at: expectString(object.created_at, 'identity mapping created_at'),
    updated_at: expectString(object.updated_at, 'identity mapping updated_at'),
  };
}

function toIdentityResourceCounts(payload: unknown): IdentityResourceCounts {
  const object = expectRecord(payload, 'identity resource counts');
  return {
    projects: expectCount(object.projects, 'identity resource counts projects'),
    service_principals: expectCount(object.service_principals, 'identity resource counts service_principals'),
    identity_mappings: expectCount(object.identity_mappings, 'identity resource counts identity_mappings'),
    sessions: expectCount(object.sessions, 'identity resource counts sessions'),
    active_sessions: expectCount(object.active_sessions, 'identity resource counts active_sessions'),
    session_templates: expectCount(object.session_templates, 'identity resource counts session_templates'),
    browser_contexts: expectCount(object.browser_contexts, 'identity resource counts browser_contexts'),
    egress_profiles: expectCount(object.egress_profiles, 'identity resource counts egress_profiles'),
    credential_bindings: expectCount(object.credential_bindings, 'identity resource counts credential_bindings'),
    file_workspaces: expectCount(object.file_workspaces, 'identity resource counts file_workspaces'),
    workflow_definitions: expectCount(object.workflow_definitions, 'identity resource counts workflow_definitions'),
    workflow_runs: expectCount(object.workflow_runs, 'identity resource counts workflow_runs'),
    active_workflow_runs: expectCount(object.active_workflow_runs, 'identity resource counts active_workflow_runs'),
    automation_tasks: expectCount(object.automation_tasks, 'identity resource counts automation_tasks'),
    active_automation_tasks: expectCount(
      object.active_automation_tasks,
      'identity resource counts active_automation_tasks',
    ),
    extension_definitions: expectCount(object.extension_definitions, 'identity resource counts extension_definitions'),
    delegated_principals: expectCount(object.delegated_principals, 'identity resource counts delegated_principals'),
  };
}

function toIdentityMappingReviewResource(payload: unknown): IdentityMappingReviewResource {
  const object = expectRecord(payload, 'identity mapping review');
  return {
    ...toIdentityMappingResource(object),
    effective_for_principal: expectBoolean(
      object.effective_for_principal,
      'identity mapping effective_for_principal',
    ),
  };
}

function toIdentityServicePrincipalReviewResource(payload: unknown): IdentityServicePrincipalReviewResource {
  const object = expectRecord(payload, 'identity service principal review');
  return {
    ...toServicePrincipalResource(object),
    delegated_session_count: expectCount(
      object.delegated_session_count,
      'identity service principal delegated_session_count',
    ),
    active_delegated_session_count: expectCount(
      object.active_delegated_session_count,
      'identity service principal active_delegated_session_count',
    ),
    delegated_session_ids: toStringArray(
      object.delegated_session_ids,
      'identity service principal delegated_session_ids',
    ),
  };
}

function toIdentityDelegatedPrincipalResource(payload: unknown): IdentityDelegatedPrincipalResource {
  const object = expectRecord(payload, 'identity delegated principal');
  return {
    client_id: expectString(object.client_id, 'identity delegated principal client_id'),
    issuer: expectString(object.issuer, 'identity delegated principal issuer'),
    display_name: optionalString(object.display_name, 'identity delegated principal display_name') ?? null,
    registered: expectBoolean(object.registered, 'identity delegated principal registered'),
    registered_service_principal_id: optionalString(
      object.registered_service_principal_id,
      'identity delegated principal registered_service_principal_id',
    ) ?? null,
    state: optionalEnum(object.state, SERVICE_PRINCIPAL_STATES, 'identity delegated principal state') ?? null,
    session_count: expectCount(object.session_count, 'identity delegated principal session_count'),
    active_session_count: expectCount(
      object.active_session_count,
      'identity delegated principal active_session_count',
    ),
    session_ids: toStringArray(object.session_ids, 'identity delegated principal session_ids'),
  };
}

function toIdentityUnmappedPrincipalSignalResource(payload: unknown): IdentityUnmappedPrincipalSignalResource {
  const object = expectRecord(payload, 'identity unmapped principal signal');
  return {
    kind: expectEnum(object.kind, IDENTITY_MAPPING_KINDS, 'identity unmapped principal signal kind'),
    issuer: expectString(object.issuer, 'identity unmapped principal signal issuer'),
    external_id: expectString(object.external_id, 'identity unmapped principal signal external_id'),
    claim_name: optionalString(object.claim_name, 'identity unmapped principal signal claim_name') ?? null,
    display_name: optionalString(object.display_name, 'identity unmapped principal signal display_name') ?? null,
    reason: expectString(object.reason, 'identity unmapped principal signal reason'),
  };
}

function jsonRequest(method: 'POST' | 'PUT', body: unknown): RequestInit {
  return {
    method,
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new IdentityCatalogError(`${label} must be an object.`, 'invalid_payload');
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new IdentityCatalogError(`${label} must be an array.`, 'invalid_payload');
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new IdentityCatalogError(`${label} must be a non-empty string.`, 'invalid_payload');
  }
  return value;
}

function optionalString(value: unknown, label: string): string | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  if (typeof value !== 'string') {
    throw new IdentityCatalogError(`${label} must be a string.`, 'invalid_payload');
  }
  return value;
}

function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') {
    throw new IdentityCatalogError(`${label} must be a boolean.`, 'invalid_payload');
  }
  return value;
}

function expectCount(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0) {
    throw new IdentityCatalogError(`${label} must be a non-negative integer.`, 'invalid_payload');
  }
  return value;
}

function expectEnum<T extends string>(value: unknown, allowed: readonly T[], label: string): T {
  if (typeof value !== 'string' || !allowed.includes(value as T)) {
    throw new IdentityCatalogError(`${label} has an unsupported value.`, 'invalid_payload');
  }
  return value as T;
}

function optionalEnum<T extends string>(value: unknown, allowed: readonly T[], label: string): T | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  return expectEnum(value, allowed, label);
}

function toStringArray(value: unknown, label: string): readonly string[] {
  return expectArray(value, label).map((entry) => expectString(entry, `${label} entry`));
}

function toStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
  const object = expectRecord(value, label);
  return Object.fromEntries(
    Object.entries(object).map(([key, entry]) => [key, expectString(entry, `${label} ${key}`)]),
  );
}
