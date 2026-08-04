import type { AuthConfig } from './auth-config';
import { BrowserTokenStore } from './browser-token-store';
import { OidcEndpointClient } from './oidc-endpoint-client';
import { OidcIdTokenVerifier } from './oidc-id-token-verifier';
import { OidcLoginTransaction } from './oidc-login-transaction';
import type { AuthSnapshot, OidcClaims, OidcTokenSet } from './oidc-types';
import { OidcWireMapper } from './oidc-wire-mapper';
import { PkceCodec } from './pkce-codec';

const TOKEN_REFRESH_SKEW_MS = 60_000;

export type LoginCompletion = {
  readonly completed: boolean;
  readonly cleanUrl: string;
};

export type OidcAuthClientDependencies = {
  readonly config: AuthConfig;
  readonly tokenStore: BrowserTokenStore;
  readonly endpoints: OidcEndpointClient;
  readonly verifier: OidcIdTokenVerifier;
  readonly loginTransaction: OidcLoginTransaction;
  readonly cryptoImpl: Crypto;
  readonly nowMs: () => number;
};

export class OidcAuthClient {
  readonly #config: AuthConfig;
  readonly #tokenStore: BrowserTokenStore;
  readonly #endpoints: OidcEndpointClient;
  readonly #verifier: OidcIdTokenVerifier;
  readonly #loginTransaction: OidcLoginTransaction;
  readonly #cryptoImpl: Crypto;
  readonly #nowMs: () => number;
  #tokens: OidcTokenSet | null;
  #claims: OidcClaims | null = null;

  constructor(dependencies: OidcAuthClientDependencies) {
    this.#config = dependencies.config;
    this.#tokenStore = dependencies.tokenStore;
    this.#endpoints = dependencies.endpoints;
    this.#verifier = dependencies.verifier;
    this.#loginTransaction = dependencies.loginTransaction;
    this.#cryptoImpl = dependencies.cryptoImpl;
    this.#nowMs = dependencies.nowMs;
    this.#tokens = this.#tokenStore.loadTokens();
  }

  async initialize(): Promise<void> {
    if (this.#claims) {
      return;
    }
    if (!this.#tokens?.id_token || this.#nowMs() >= this.#tokens.expiresAtMs) {
      this.clear();
      return;
    }
    try {
      this.#claims = await this.#verifier.verify(this.#tokens.id_token);
    } catch {
      this.clear();
    }
  }

  getSnapshot(): AuthSnapshot {
    const username = this.#claims?.preferred_username
      ?? this.#claims?.email
      ?? this.#config.exampleUser?.username
      ?? this.#claims?.sub
      ?? '--';
    return {
      configured: this.#config.mode === 'oidc',
      authenticated: Boolean(this.#tokens?.access_token && this.#claims),
      username,
      accessToken: this.#claims ? this.#tokens?.access_token ?? null : null,
      claims: this.#claims,
    };
  }

  async buildLoginUrl(currentUrl: URL): Promise<string> {
    this.#assertConfigured();
    const metadata = await this.#endpoints.fetchMetadata();
    const pkce = this.#loginTransaction.create(currentUrl);
    const url = new URL(metadata.authorization_endpoint);
    url.searchParams.set('client_id', this.#endpoints.clientId());
    url.searchParams.set('redirect_uri', pkce.redirectUri);
    url.searchParams.set('response_type', 'code');
    url.searchParams.set('scope', this.#scope());
    url.searchParams.set('state', pkce.state);
    url.searchParams.set('nonce', pkce.nonce);
    url.searchParams.set('code_challenge', await PkceCodec.sha256Base64Url(this.#cryptoImpl, pkce.verifier));
    url.searchParams.set('code_challenge_method', 'S256');
    return url.toString();
  }

  async completeLoginIfNeeded(currentUrl: URL): Promise<LoginCompletion> {
    this.#assertConfigured();
    const cleanUrl = PkceCodec.buildRedirectUri(currentUrl);
    if (currentUrl.searchParams.has('error')) {
      this.clear();
      throw new Error('OIDC authorization failed');
    }
    const code = currentUrl.searchParams.get('code');
    if (!code) {
      return { completed: false, cleanUrl };
    }
    const pkce = this.#loginTransaction.consume(currentUrl.searchParams.get('state'));
    try {
      const response = await this.#endpoints.exchangeAuthorizationCode(code, pkce);
      await this.#saveTokenResponse(response, pkce.nonce);
      return { completed: true, cleanUrl };
    } catch (error) {
      this.clear();
      throw error;
    }
  }

  async getValidAccessToken(): Promise<string | null> {
    if (!this.#tokens?.access_token || !this.#claims) {
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
    return this.#config.mode === 'oidc'
      ? await this.#endpoints.buildLogoutUrl(idToken, currentUrl)
      : null;
  }

  clear(): void {
    this.#tokens = null;
    this.#claims = null;
    this.#tokenStore.clearTokens();
    this.#tokenStore.clearPkceState();
  }

  async #refreshAccessToken(): Promise<string | null> {
    const previous = this.#tokens;
    const refreshToken = previous?.refresh_token;
    if (!refreshToken) {
      this.clear();
      return null;
    }
    try {
      const response = await this.#endpoints.refreshAccessToken(refreshToken);
      if (!response) {
        this.clear();
        return null;
      }
      await this.#saveTokenResponse(response, undefined, previous);
      return this.#tokens?.access_token ?? null;
    } catch {
      this.clear();
      return null;
    }
  }

  async #saveTokenResponse(payload: unknown, nonce?: string, previous?: OidcTokenSet): Promise<void> {
    const tokens = OidcWireMapper.toTokenSet(payload, this.#nowMs(), previous);
    if (!tokens.id_token) {
      throw new Error('OIDC ID token is required');
    }
    const claims = await this.#verifier.verify(tokens.id_token, nonce);
    this.#tokens = tokens;
    this.#claims = claims;
    this.#tokenStore.saveTokens(tokens);
  }

  #scope(): string {
    const scope = this.#config.scope ?? 'openid';
    if (!scope.split(/\s+/).includes('openid')) {
      throw new Error('OIDC scope must include openid');
    }
    return scope;
  }

  #assertConfigured(): void {
    if (this.#config.mode !== 'oidc') {
      throw new Error('OIDC auth is not configured');
    }
  }
}
