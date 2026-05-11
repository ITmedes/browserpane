import type { AuthConfig } from './auth-config';
import { BrowserTokenStore } from './browser-token-store';
import { OidcEndpointClient } from './oidc-endpoint-client';
import { OidcWireMapper } from './oidc-wire-mapper';
import { PkceCodec } from './pkce-codec';
import type { AuthSnapshot, OidcClaims, OidcTokenSet } from './oidc-types';

const TOKEN_REFRESH_SKEW_MS = 60_000;

export type OidcAuthClientOptions = {
  readonly config: AuthConfig;
  readonly tokenStore: BrowserTokenStore;
  readonly fetchImpl?: typeof fetch;
  readonly cryptoImpl?: Crypto;
  readonly nowMs?: () => number;
};

export type LoginCompletion = {
  readonly completed: boolean;
  readonly cleanUrl: string;
};

export class OidcAuthClient {
  readonly #config: AuthConfig;
  readonly #tokenStore: BrowserTokenStore;
  readonly #fetchImpl: typeof fetch;
  readonly #cryptoImpl: Crypto;
  readonly #nowMs: () => number;
  readonly #endpoints: OidcEndpointClient;
  #tokens: OidcTokenSet | null = null;
  #claims: OidcClaims | null = null;

  constructor(options: OidcAuthClientOptions) {
    this.#config = options.config;
    this.#tokenStore = options.tokenStore;
    this.#fetchImpl = options.fetchImpl ?? fetch;
    this.#cryptoImpl = options.cryptoImpl ?? crypto;
    this.#nowMs = options.nowMs ?? Date.now;
    this.#endpoints = new OidcEndpointClient(this.#config, this.#fetchImpl);
    this.#loadStoredTokens();
  }

  getSnapshot(): AuthSnapshot {
    const username =
      this.#claims?.preferred_username
      ?? this.#claims?.email
      ?? this.#config.exampleUser?.username
      ?? this.#claims?.sub
      ?? '--';
    return {
      configured: this.#config.mode === 'oidc',
      authenticated: Boolean(this.#tokens?.access_token),
      username,
      accessToken: this.#tokens?.access_token ?? null,
      claims: this.#claims,
    };
  }

  async buildLoginUrl(currentUrl: URL): Promise<string> {
    this.#assertConfigured();
    const metadata = await this.#endpoints.fetchMetadata();
    const verifier = PkceCodec.randomString(this.#cryptoImpl, 48);
    const state = PkceCodec.randomString(this.#cryptoImpl, 24);
    const redirectUri = PkceCodec.buildRedirectUri(currentUrl);
    const challenge = await PkceCodec.sha256Base64Url(this.#cryptoImpl, verifier);
    this.#tokenStore.savePkceState({ verifier, state, redirectUri });

    const url = new URL(metadata.authorization_endpoint);
    url.searchParams.set('client_id', this.#endpoints.clientId());
    url.searchParams.set('redirect_uri', redirectUri);
    url.searchParams.set('response_type', 'code');
    url.searchParams.set('scope', this.#config.scope ?? 'openid');
    url.searchParams.set('state', state);
    url.searchParams.set('code_challenge', challenge);
    url.searchParams.set('code_challenge_method', 'S256');
    return url.toString();
  }

  async completeLoginIfNeeded(currentUrl: URL): Promise<LoginCompletion> {
    this.#assertConfigured();
    const params = currentUrl.searchParams;
    const code = params.get('code');
    const cleanUrl = PkceCodec.buildRedirectUri(currentUrl);
    if (!code) {
      return { completed: false, cleanUrl };
    }

    const pkce = this.#tokenStore.loadPkceState();
    if (!pkce) {
      throw new Error('Missing PKCE login state');
    }
    if (pkce.state !== params.get('state')) {
      throw new Error('OIDC state mismatch');
    }

    this.#saveTokenResponse(await this.#endpoints.exchangeAuthorizationCode(code, pkce));
    this.#tokenStore.clearPkceState();
    return { completed: true, cleanUrl };
  }

  async getValidAccessToken(): Promise<string | null> {
    if (!this.#tokens?.access_token) {
      return null;
    }
    if (this.#nowMs() < this.#tokens.expiresAtMs - TOKEN_REFRESH_SKEW_MS) {
      return this.#tokens.access_token;
    }
    return await this.#refreshAccessToken();
  }

  async buildLogoutUrl(currentUrl: URL): Promise<string | null> {
    const idToken = this.#tokens?.id_token;
    this.clear();
    if (this.#config.mode !== 'oidc') {
      return null;
    }
    return await this.#endpoints.buildLogoutUrl(idToken, currentUrl);
  }

  clear(): void {
    this.#tokens = null;
    this.#claims = null;
    this.#tokenStore.clearTokens();
    this.#tokenStore.clearPkceState();
  }

  async #refreshAccessToken(): Promise<string | null> {
    if (!this.#tokens?.refresh_token) {
      this.clear();
      return null;
    }
    const response = await this.#endpoints.refreshAccessToken(this.#tokens.refresh_token);
    if (!response.ok) {
      this.clear();
      return null;
    }
    this.#saveTokenResponse({
      ...this.#tokens,
      ...(await response.json()),
    });
    return this.#tokens?.access_token ?? null;
  }

  #loadStoredTokens(): void {
    try {
      this.#tokens = this.#tokenStore.loadTokens();
      this.#claims =
        PkceCodec.parseJwtClaims(this.#tokens?.id_token)
        ?? PkceCodec.parseJwtClaims(this.#tokens?.access_token);
    } catch {
      this.clear();
    }
  }

  #saveTokenResponse(payload: unknown): void {
    const tokens = OidcWireMapper.toTokenSet(payload, this.#nowMs());
    this.#tokens = tokens;
    this.#claims =
      PkceCodec.parseJwtClaims(tokens.id_token)
      ?? PkceCodec.parseJwtClaims(tokens.access_token);
    this.#tokenStore.saveTokens(tokens);
  }

  #assertConfigured(): void {
    if (this.#config.mode !== 'oidc') {
      throw new Error('OIDC auth is not configured');
    }
  }
}
