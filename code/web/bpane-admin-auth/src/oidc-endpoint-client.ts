import type { AuthConfig } from './auth-config';
import type { OidcMetadata, PkceState } from './oidc-types';
import { OidcWireMapper } from './oidc-wire-mapper';
import { PkceCodec } from './pkce-codec';

export class OidcEndpointClient {
  readonly #config: AuthConfig;
  readonly #fetchImpl: typeof fetch;
  #metadata: OidcMetadata | null = null;

  constructor(config: AuthConfig, fetchImpl: typeof fetch) {
    this.#config = config;
    this.#fetchImpl = fetchImpl;
  }

  async fetchMetadata(): Promise<OidcMetadata> {
    if (this.#metadata) {
      return this.#metadata;
    }
    const issuer = this.issuer();
    const response = await this.#fetchImpl(`${issuer}/.well-known/openid-configuration`, {
      cache: 'no-store',
    });
    if (!response.ok) {
      throw new Error(`OIDC discovery failed with HTTP ${response.status}`);
    }
    const metadata = OidcWireMapper.toMetadata(await response.json());
    if (normalizeIssuer(metadata.issuer) !== issuer) {
      throw new Error('OIDC discovery issuer mismatch');
    }
    this.#metadata = metadata;
    return metadata;
  }

  async exchangeAuthorizationCode(code: string, pkce: PkceState): Promise<unknown> {
    const metadata = await this.fetchMetadata();
    const body = new URLSearchParams({
      grant_type: 'authorization_code',
      client_id: this.clientId(),
      code,
      redirect_uri: pkce.redirectUri,
      code_verifier: pkce.verifier,
    });
    const response = await this.#postTokenRequest(metadata.token_endpoint, body);
    if (!response.ok) {
      throw new Error(`OIDC code exchange failed with HTTP ${response.status}`);
    }
    return await response.json();
  }

  async refreshAccessToken(refreshToken: string): Promise<unknown | null> {
    const metadata = await this.fetchMetadata();
    const response = await this.#postTokenRequest(metadata.token_endpoint, new URLSearchParams({
      grant_type: 'refresh_token',
      client_id: this.clientId(),
      refresh_token: refreshToken,
    }));
    return response.ok ? await response.json() : null;
  }

  async buildLogoutUrl(idToken: string | undefined, currentUrl: URL): Promise<string> {
    const metadata = await this.fetchMetadata();
    if (!metadata.end_session_endpoint) {
      return PkceCodec.buildRedirectUri(currentUrl);
    }
    const url = new URL(metadata.end_session_endpoint);
    url.searchParams.set('post_logout_redirect_uri', PkceCodec.buildRedirectUri(currentUrl));
    url.searchParams.set('client_id', this.clientId());
    if (idToken) {
      url.searchParams.set('id_token_hint', idToken);
    }
    return url.toString();
  }

  clientId(): string {
    return requiredString(this.#config.clientId, 'OIDC client id');
  }

  issuer(): string {
    return normalizeIssuer(requiredString(this.#config.issuer, 'OIDC issuer'));
  }

  fetchImpl(): typeof fetch {
    return this.#fetchImpl;
  }

  async #postTokenRequest(tokenEndpoint: string, body: URLSearchParams): Promise<Response> {
    return await this.#fetchImpl(tokenEndpoint, {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body,
    });
  }
}

function requiredString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
}

function normalizeIssuer(value: string): string {
  return value.replace(/\/$/, '');
}
