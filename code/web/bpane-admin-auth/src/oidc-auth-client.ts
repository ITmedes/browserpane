import type { AuthConfig } from './auth-config';
import { BrowserTokenStore } from './browser-token-store';
import { OidcEndpointClient } from './oidc-endpoint-client';
import { OidcLoginTransaction } from './oidc-login-transaction';
import type { AuthSnapshot, OidcClaims, OidcExchangeResult, OidcTokenSet } from './oidc-types';
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
  readonly loginTransaction: OidcLoginTransaction;
  readonly nowMs: () => number;
};

export class OidcAuthClient {
  readonly #config: AuthConfig;
  readonly #tokenStore: BrowserTokenStore;
  readonly #endpoints: OidcEndpointClient;
  readonly #loginTransaction: OidcLoginTransaction;
  readonly #nowMs: () => number;
  #tokens: OidcTokenSet | null = null;
  #claims: OidcClaims | null = null;

  constructor(dependencies: OidcAuthClientDependencies) {
    this.#config = dependencies.config;
    this.#tokenStore = dependencies.tokenStore;
    this.#endpoints = dependencies.endpoints;
    this.#loginTransaction = dependencies.loginTransaction;
    this.#nowMs = dependencies.nowMs;
  }

  async initialize(): Promise<void> {
    this.#tokenStore.clearTokens();
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
    const pkce = this.#loginTransaction.create(currentUrl);
    return await this.#endpoints.buildAuthorizationUrl(pkce, this.#scope());
  }

  async completeLoginIfNeeded(currentUrl: URL): Promise<LoginCompletion> {
    this.#assertConfigured();
    const cleanUrl = PkceCodec.buildRedirectUri(currentUrl);
    const code = currentUrl.searchParams.get('code');
    if (!code && !currentUrl.searchParams.has('error')) {
      return { completed: false, cleanUrl };
    }
    const pkce = this.#loginTransaction.consume();
    try {
      this.#saveExchange(await this.#endpoints.exchangeAuthorizationCode(currentUrl, pkce));
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
      this.#saveExchange(await this.#endpoints.refreshAccessToken(refreshToken, previous), this.#claims);
      return this.#tokens?.access_token ?? null;
    } catch {
      this.clear();
      return null;
    }
  }

  #saveExchange(result: OidcExchangeResult, fallbackClaims?: OidcClaims | null): void {
    const claims = result.claims ?? fallbackClaims;
    if (!claims) {
      throw new Error('OIDC ID token claims are required');
    }
    this.#tokens = result.tokens;
    this.#claims = claims;
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
