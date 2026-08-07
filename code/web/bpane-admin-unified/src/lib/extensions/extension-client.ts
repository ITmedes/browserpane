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
  CreateExtensionDefinitionRequest,
  CreateExtensionVersionRequest,
  ExtensionDefinitionListResponse,
  ExtensionDefinitionResource,
  ExtensionVersionResource,
} from './extension-types';

export type { AccessTokenProvider, FetchLike } from '$lib/api/authenticated-api';
export type ExtensionCatalogErrorCode = AdminApiRequestErrorCode;

export type ExtensionCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class ExtensionCatalogError extends AdminApiRequestError {
  constructor(
    message: string,
    code: ExtensionCatalogErrorCode,
    status: number | null = null,
    failure?: AdminApiRequestFailure,
  ) {
    super(message, failure ?? { code, status, message });
    this.name = 'ExtensionCatalogError';
  }
}

export class ExtensionCatalogClient {
  readonly #baseUrl: URL;
  readonly #api: AuthenticatedApiClient;

  constructor(options: ExtensionCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#api = new AuthenticatedApiClient({
      baseUrl: this.#baseUrl,
      accessTokenProvider: options.accessTokenProvider,
      ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
      ...(options.onAuthenticationFailure === undefined
        ? {}
        : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) => new ExtensionCatalogError(
        formatAdminApiRequestError('Extension catalog request', failure),
        failure.code,
        failure.status,
        failure,
      ),
    });
  }

  async listExtensions(): Promise<ExtensionDefinitionListResponse> {
    const response = await this.#request('/api/v1/extensions', { method: 'GET' });
    return toExtensionDefinitionListResponse(await response.json());
  }

  async createExtension(
    request: CreateExtensionDefinitionRequest,
  ): Promise<ExtensionDefinitionResource> {
    const response = await this.#request('/api/v1/extensions', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(request),
    });
    return toExtensionDefinitionResource(await response.json());
  }

  async getExtension(extensionId: string): Promise<ExtensionDefinitionResource> {
    const response = await this.#request(`/api/v1/extensions/${encodeURIComponent(extensionId)}`, {
      method: 'GET',
    });
    return toExtensionDefinitionResource(await response.json());
  }

  async createExtensionVersion(
    extensionId: string,
    request: CreateExtensionVersionRequest,
  ): Promise<ExtensionVersionResource> {
    const response = await this.#request(
      `/api/v1/extensions/${encodeURIComponent(extensionId)}/versions`,
      {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(request),
      },
    );
    return toExtensionVersionResource(await response.json());
  }

  async setExtensionEnabled(extensionId: string, enabled: boolean): Promise<ExtensionDefinitionResource> {
    const transition = enabled ? 'enable' : 'disable';
    const response = await this.#request(
      `/api/v1/extensions/${encodeURIComponent(extensionId)}/${transition}`,
      { method: 'POST' },
    );
    return toExtensionDefinitionResource(await response.json());
  }

  async #request(path: string, init: RequestInit): Promise<Response> {
    const headers = new Headers(init.headers);
    headers.set('accept', 'application/json');
    return await this.#api.request(new URL(path, this.#baseUrl), { ...init, headers });
  }
}

export function toExtensionDefinitionListResponse(payload: unknown): ExtensionDefinitionListResponse {
  const object = expectRecord(payload, 'extension list response');
  return {
    extensions: expectArray(object.extensions, 'extension list extensions')
      .map(toExtensionDefinitionResource),
  };
}

export function toExtensionDefinitionResource(value: unknown): ExtensionDefinitionResource {
  const object = expectRecord(value, 'extension definition');
  return {
    id: expectString(object.id, 'extension id'),
    name: expectString(object.name, 'extension name'),
    description: optionalString(object.description, 'extension description') ?? null,
    enabled: expectBoolean(object.enabled, 'extension enabled'),
    latest_version: optionalString(object.latest_version, 'extension latest_version') ?? null,
    labels: toStringRecord(object.labels, 'extension labels'),
    created_at: expectString(object.created_at, 'extension created_at'),
    updated_at: expectString(object.updated_at, 'extension updated_at'),
  };
}

export function toExtensionVersionResource(value: unknown): ExtensionVersionResource {
  const object = expectRecord(value, 'extension version');
  return {
    id: expectString(object.id, 'extension version id'),
    extension_definition_id: expectString(
      object.extension_definition_id,
      'extension version extension_definition_id',
    ),
    version: expectString(object.version, 'extension version'),
    install_path: expectString(object.install_path, 'extension version install_path'),
    created_at: expectString(object.created_at, 'extension version created_at'),
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new ExtensionCatalogError(`${label} must be an object.`, 'invalid_payload');
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new ExtensionCatalogError(`${label} must be an array.`, 'invalid_payload');
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string') {
    throw new ExtensionCatalogError(`${label} must be a string.`, 'invalid_payload');
  }
  return value;
}

function optionalString(value: unknown, label: string): string | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return expectString(value, label);
}

function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') {
    throw new ExtensionCatalogError(`${label} must be a boolean.`, 'invalid_payload');
  }
  return value;
}

function toStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
  const object = expectRecord(value, label);
  return Object.fromEntries(
    Object.entries(object).map(([key, item]) => [key, expectString(item, `${label}.${key}`)]),
  );
}
