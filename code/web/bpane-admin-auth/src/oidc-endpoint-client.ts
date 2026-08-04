import * as oauth from 'oauth4webapi';
import type { AuthConfig } from './auth-config';
import type { OidcExchangeResult, OidcTokenSet, PkceState } from './oidc-types';
import { OidcWireMapper } from './oidc-wire-mapper';
import { PkceCodec } from './pkce-codec';

export class OidcEndpointClient {
  readonly #client: oauth.Client;
  readonly #fetchImpl: typeof fetch;
  readonly #issuer: URL;
  readonly #nowMs: () => number;
  readonly #allowInsecure: boolean;
  #metadata: oauth.AuthorizationServer | null = null;

  constructor(config: AuthConfig, fetchImpl: typeof fetch, nowMs: () => number) {
    this.#issuer = issuerUrl(requiredString(config.issuer, 'OIDC issuer'));
    this.#fetchImpl = fetchImpl;
    this.#nowMs = nowMs;
    this.#allowInsecure = this.#issuer.protocol === 'http:';
    this.#client = {
      client_id: requiredString(config.clientId, 'OIDC client id'),
      [oauth.clockSkew]: (nowMs() - Date.now()) / 1000,
      [oauth.clockTolerance]: 5,
    };
  }

  async fetchMetadata(): Promise<oauth.AuthorizationServer> {
    if (this.#metadata) {
      return this.#metadata;
    }
    const response = await oauth.discoveryRequest(this.#issuer, this.#requestOptions());
    this.#metadata = await oauth.processDiscoveryResponse(this.#issuer, response);
    return this.#metadata;
  }

  async buildAuthorizationUrl(pkce: PkceState, scope: string): Promise<string> {
    const metadata = await this.fetchMetadata();
    const endpoint = requiredString(metadata.authorization_endpoint, 'OIDC authorization endpoint');
    const url = new URL(endpoint);
    url.searchParams.set('client_id', this.clientId());
    url.searchParams.set('redirect_uri', pkce.redirectUri);
    url.searchParams.set('response_type', 'code');
    url.searchParams.set('scope', scope);
    url.searchParams.set('state', pkce.state);
    url.searchParams.set('nonce', pkce.nonce);
    url.searchParams.set('code_challenge', await oauth.calculatePKCECodeChallenge(pkce.verifier));
    url.searchParams.set('code_challenge_method', 'S256');
    return url.toString();
  }

  async exchangeAuthorizationCode(currentUrl: URL, pkce: PkceState): Promise<OidcExchangeResult> {
    const metadata = await this.fetchMetadata();
    const parameters = oauth.validateAuthResponse(metadata, this.#client, currentUrl, pkce.state);
    const response = await oauth.authorizationCodeGrantRequest(
      metadata,
      this.#client,
      oauth.None(),
      parameters,
      pkce.redirectUri,
      pkce.verifier,
      this.#requestOptions(),
    );
    const result = await oauth.processAuthorizationCodeResponse(metadata, this.#client, response, {
      expectedNonce: pkce.nonce,
      requireIdToken: true,
    });
    await oauth.validateApplicationLevelSignature(metadata, response, this.#requestOptions());
    const claims = oauth.getValidatedIdTokenClaims(result);
    if (!claims) {
      throw new Error('OIDC ID token is required');
    }
    return {
      tokens: OidcWireMapper.toTokenSet(result, this.#nowMs()),
      claims: OidcWireMapper.toClaims(claims),
    };
  }

  async refreshAccessToken(refreshToken: string, previous: OidcTokenSet): Promise<OidcExchangeResult> {
    const metadata = await this.fetchMetadata();
    const response = await oauth.refreshTokenGrantRequest(
      metadata,
      this.#client,
      oauth.None(),
      refreshToken,
      this.#requestOptions(),
    );
    const result = await oauth.processRefreshTokenResponse(metadata, this.#client, response);
    if (result.id_token) {
      await oauth.validateApplicationLevelSignature(metadata, response, this.#requestOptions());
    }
    const claims = oauth.getValidatedIdTokenClaims(result);
    return {
      tokens: OidcWireMapper.toTokenSet(result, this.#nowMs(), previous),
      ...(claims ? { claims: OidcWireMapper.toClaims(claims) } : {}),
    };
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
    return this.#client.client_id;
  }

  #requestOptions() {
    return {
      [oauth.customFetch]: this.#customFetch,
      ...(this.#allowInsecure ? { [oauth.allowInsecureRequests]: true } : {}),
    };
  }

  #customFetch = <Method, BodyType>(
    url: string,
    options: oauth.CustomFetchOptions<Method, BodyType>,
  ): Promise<Response> => {
    const init: RequestInit = {
      headers: options.headers,
      method: String(options.method),
      redirect: options.redirect,
      ...(options.signal ? { signal: options.signal } : {}),
      ...(options.body !== undefined ? { body: options.body as BodyInit } : {}),
    };
    return this.#fetchImpl(url, init);
  };
}

function issuerUrl(value: string): URL {
  const url = new URL(value.replace(/\/$/, ''));
  if (url.protocol === 'https:') {
    return url;
  }
  if (url.protocol === 'http:' && isLoopbackHost(url.hostname)) {
    return url;
  }
  throw new Error('OIDC issuer must use HTTPS unless it is a loopback development endpoint');
}

function isLoopbackHost(hostname: string): boolean {
  return hostname === 'localhost' || hostname === '127.0.0.1' || hostname === '[::1]';
}

function requiredString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
}
