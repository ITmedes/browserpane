import {
  AuthenticatedApiClient,
  formatAdminApiRequestError,
  type AccessTokenProvider,
  type FetchLike,
} from '$lib/api/authenticated-api';
import type {
  CreateCredentialBindingRequest,
  CredentialBindingListResponse,
  CredentialBindingProjectOptionsResponse,
  CredentialBindingResource,
} from './credential-binding-types';
import { CredentialBindingCatalogError } from './credential-binding-error';
import { CredentialBindingWireMapper } from './credential-binding-wire-mapper';

export { CredentialBindingCatalogError } from './credential-binding-error';

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
      ...(options.onAuthenticationFailure === undefined
        ? {}
        : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) =>
        new CredentialBindingCatalogError(
          formatAdminApiRequestError('Credential binding catalog request', failure),
          failure.code,
          failure.status,
          failure,
        ),
    });
  }

  async listCredentialBindings(): Promise<CredentialBindingListResponse> {
    const response = await this.#request('/api/v1/credential-bindings', { method: 'GET' });
    return CredentialBindingWireMapper.toListResponse(await response.json());
  }

  async createCredentialBinding(
    request: CreateCredentialBindingRequest,
  ): Promise<CredentialBindingResource> {
    const response = await this.#request('/api/v1/credential-bindings', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(request),
    });
    return CredentialBindingWireMapper.toResource(await response.json());
  }

  async getCredentialBinding(bindingId: string): Promise<CredentialBindingResource> {
    const response = await this.#request(
      `/api/v1/credential-bindings/${encodeURIComponent(bindingId)}`,
      { method: 'GET' },
    );
    return CredentialBindingWireMapper.toResource(await response.json());
  }

  async listProjectOptions(): Promise<CredentialBindingProjectOptionsResponse> {
    const response = await this.#request('/api/v1/projects', { method: 'GET' });
    return CredentialBindingWireMapper.toProjectOptions(await response.json());
  }

  async #request(path: string, init: RequestInit): Promise<Response> {
    const headers = new Headers(init.headers);
    headers.set('accept', 'application/json');
    return await this.#api.request(new URL(path, this.#baseUrl), { ...init, headers });
  }
}
