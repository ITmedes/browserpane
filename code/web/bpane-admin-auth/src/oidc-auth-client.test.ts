import { beforeAll, describe, expect, it, vi } from 'vitest';
import type { AuthConfig } from './auth-config';
import { BrowserTokenStore } from './browser-token-store';
import { OidcAuthClientFactory } from './oidc-auth-client-factory';
import { MemoryStorage } from './test-support/memory-storage';
import { OidcTokenFixture, type OidcTokenOverrides } from './test-support/oidc-token-fixture';

const ISSUER = 'https://identity.example.test/realms/browserpane';
const CLIENT_ID = 'bpane-web';
const NOW_SECONDS = 1_700_000_000;
const CONFIG: AuthConfig = {
  mode: 'oidc',
  issuer: ISSUER,
  clientId: CLIENT_ID,
  scope: 'openid profile email',
};
const METADATA = {
  issuer: ISSUER,
  authorization_endpoint: `${ISSUER}/authorize`,
  token_endpoint: `${ISSUER}/token`,
  jwks_uri: `${ISSUER}/jwks`,
  end_session_endpoint: `${ISSUER}/logout`,
  id_token_signing_alg_values_supported: ['RS256'],
};

describe('OidcAuthClient', () => {
  let fixture: OidcTokenFixture;

  beforeAll(async () => {
    fixture = await OidcTokenFixture.create(ISSUER, CLIENT_ID);
  });

  it('completes a signed nonce-bound login while keeping every token in memory', async () => {
    const storage = new MemoryStorage();
    let tokenResponse: unknown = null;
    const client = newClient(storage, createProviderFetch(() => tokenResponse, fixture));
    const loginUrl = new URL(await client.buildLoginUrl(new URL('https://app.example.test/admin-new/projects')));
    tokenResponse = await validTokenResponse(fixture, loginUrl.searchParams.get('nonce'));

    const completion = await client.completeLoginIfNeeded(callbackUrl(loginUrl));

    expect(completion).toEqual({
      completed: true,
      cleanUrl: 'https://app.example.test/admin-new/projects',
    });
    expect(client.getSnapshot()).toMatchObject({ authenticated: true, username: 'operator' });
    expect(await client.getValidAccessToken()).toBe('access-token');
    expect(storage.getItem('bpane.admin.auth.tokens.v2')).toBeNull();
    expect(storage.getItem('bpane.admin.auth.tokens.v1')).toBeNull();
    expect(storage.getItem('bpane.admin.auth.pkce.v2')).toBeNull();
  });

  it('does not restore credentials after reload and removes legacy token state', async () => {
    const storage = new MemoryStorage();
    storage.setItem('bpane.admin.auth.tokens.v2', '{"access_token":"stored-access-token"}');
    storage.setItem('bpane.admin.auth.tokens.v1', '{"refresh_token":"stored-refresh-token"}');
    const client = newClient(storage, createProviderFetch(() => null, fixture));

    await client.initialize();

    expect(client.getSnapshot().authenticated).toBe(false);
    expect(storage.getItem('bpane.admin.auth.tokens.v2')).toBeNull();
    expect(storage.getItem('bpane.admin.auth.tokens.v1')).toBeNull();
  });

  it('delegates state validation to the certified protocol implementation', async () => {
    const storage = new MemoryStorage();
    const fetchImpl = vi.fn<typeof fetch>(createProviderFetch(() => null, fixture));
    const client = newClient(storage, fetchImpl);
    await client.buildLoginUrl(new URL('https://app.example.test/admin/'));

    await expect(client.completeLoginIfNeeded(new URL(
      'https://app.example.test/admin/?code=code&state=wrong',
    ))).rejects.toThrow(/state/i);
    expect(fetchImpl).toHaveBeenCalledTimes(1);
    expect(storage.getItem('bpane.admin.auth.pkce.v2')).toBeNull();
  });

  it.each([
    ['issuer', { iss: 'https://attacker.example.test' }],
    ['audience', { aud: 'other-client' }],
    ['expiry', { exp: NOW_SECONDS - 30 }],
    ['authorized party', { aud: [CLIENT_ID, 'secondary'], azp: 'other-client' }],
    ['nonce', { nonce: 'wrong-nonce' }],
  ] satisfies ReadonlyArray<readonly [string, OidcTokenOverrides]>)('rejects a token with invalid %s claims', async (_label, overrides) => {
    const storage = new MemoryStorage();
    let tokenResponse: unknown = null;
    const client = newClient(storage, createProviderFetch(() => tokenResponse, fixture));
    const loginUrl = new URL(await client.buildLoginUrl(new URL('https://app.example.test/admin/')));
    tokenResponse = await validTokenResponse(fixture, loginUrl.searchParams.get('nonce'), overrides);

    await expect(client.completeLoginIfNeeded(callbackUrl(loginUrl))).rejects.toThrow();
    expect(client.getSnapshot().authenticated).toBe(false);
  });

  it('rejects an ID token signed by an unknown key', async () => {
    const otherFixture = await OidcTokenFixture.create(ISSUER, CLIENT_ID);
    const storage = new MemoryStorage();
    let tokenResponse: unknown = null;
    const client = newClient(storage, createProviderFetch(() => tokenResponse, fixture));
    const loginUrl = new URL(await client.buildLoginUrl(new URL('https://app.example.test/admin/')));
    tokenResponse = await validTokenResponse(otherFixture, loginUrl.searchParams.get('nonce'));

    await expect(client.completeLoginIfNeeded(callbackUrl(loginUrl))).rejects.toThrow();
    expect(client.getSnapshot().authenticated).toBe(false);
  });

  it.each([
    ['unsigned', 'none'],
    ['symmetric-algorithm', 'HS256'],
  ])('rejects an %s ID token', async (_label, algorithm) => {
    const storage = new MemoryStorage();
    let tokenResponse: unknown = null;
    const client = newClient(storage, createProviderFetch(() => tokenResponse, fixture));
    const loginUrl = new URL(await client.buildLoginUrl(new URL('https://app.example.test/admin/')));
    tokenResponse = {
      access_token: 'access-token',
      id_token: unsafeToken(algorithm, loginUrl.searchParams.get('nonce')),
      expires_in: 300,
      token_type: 'Bearer',
    };

    await expect(client.completeLoginIfNeeded(callbackUrl(loginUrl))).rejects.toThrow();
    expect(client.getSnapshot().authenticated).toBe(false);
  });

  it('rejects discovery metadata issued for a different provider', async () => {
    const client = newClient(new MemoryStorage(), async () => jsonResponse({
      ...METADATA,
      issuer: 'https://attacker.example.test',
    }));

    await expect(client.buildLoginUrl(new URL('https://app.example.test/admin/'))).rejects.toThrow(/issuer/i);
  });

  it('refreshes an expiring access token and preserves verified identity claims', async () => {
    const storage = new MemoryStorage();
    let nowMs = NOW_SECONDS * 1000;
    let tokenResponse: unknown = null;
    const client = newClient(storage, createProviderFetch(() => tokenResponse, fixture), () => nowMs);
    const loginUrl = new URL(await client.buildLoginUrl(new URL('https://app.example.test/admin/')));
    tokenResponse = await validTokenResponse(fixture, loginUrl.searchParams.get('nonce'), {}, 61);
    await client.completeLoginIfNeeded(callbackUrl(loginUrl));
    tokenResponse = {
      access_token: 'refreshed-access-token',
      refresh_token: 'rotated-refresh-token',
      expires_in: 300,
      token_type: 'Bearer',
    };
    nowMs += 2_000;

    await expect(client.getValidAccessToken()).resolves.toBe('refreshed-access-token');
    expect(client.getSnapshot()).toMatchObject({ authenticated: true, username: 'operator' });
  });

  it('clears in-memory authentication when refresh fails', async () => {
    const storage = new MemoryStorage();
    let nowMs = NOW_SECONDS * 1000;
    let tokenResponse: unknown = null;
    let refreshFails = false;
    const fetchImpl = createProviderFetch(
      () => tokenResponse,
      fixture,
      () => refreshFails ? new Response('', { status: 401 }) : null,
    );
    const client = newClient(storage, fetchImpl, () => nowMs);
    const loginUrl = new URL(await client.buildLoginUrl(new URL('https://app.example.test/admin/')));
    tokenResponse = await validTokenResponse(fixture, loginUrl.searchParams.get('nonce'), {}, 61);
    await client.completeLoginIfNeeded(callbackUrl(loginUrl));
    refreshFails = true;
    nowMs += 2_000;

    await expect(client.getValidAccessToken()).resolves.toBeNull();
    expect(client.getSnapshot().authenticated).toBe(false);
  });

  it('rejects non-loopback plaintext issuers', () => {
    expect(() => OidcAuthClientFactory.create({
      config: { ...CONFIG, issuer: 'http://identity.example.test/realms/browserpane' },
      tokenStore: new BrowserTokenStore(new MemoryStorage()),
    })).toThrow(/HTTPS/);
  });
});

function newClient(
  storage: MemoryStorage,
  fetchImpl: typeof fetch,
  nowMs: () => number = () => NOW_SECONDS * 1000,
) {
  return OidcAuthClientFactory.create({
    config: CONFIG,
    tokenStore: new BrowserTokenStore(storage),
    fetchImpl,
    nowMs,
  });
}

function createProviderFetch(
  tokenResponse: () => unknown,
  keyFixture: OidcTokenFixture,
  tokenOverride: () => Response | null = () => null,
): typeof fetch {
  return async (input, init) => {
    const url = String(input);
    if (url.endsWith('/token')) {
      const overridden = tokenOverride();
      if (overridden) return overridden;
      expect(init?.method).toBe('POST');
      return jsonResponse(tokenResponse());
    }
    if (url.endsWith('/jwks')) {
      return jsonResponse(keyFixture.keySet());
    }
    return jsonResponse(METADATA);
  };
}

async function validTokenResponse(
  signingFixture: OidcTokenFixture,
  nonce: string | null,
  overrides: OidcTokenOverrides = {},
  expiresIn = 300,
): Promise<unknown> {
  return {
    access_token: 'access-token',
    id_token: await signingFixture.sign(NOW_SECONDS, {
      nonce,
      preferred_username: 'operator',
      ...overrides,
    }),
    refresh_token: 'memory-only-refresh-token',
    expires_in: expiresIn,
    token_type: 'Bearer',
  };
}

function callbackUrl(loginUrl: URL): URL {
  return new URL(
    `https://app.example.test/admin-new/projects?code=code&state=${loginUrl.searchParams.get('state')}`,
  );
}

function unsafeToken(algorithm: string, nonce: string | null): string {
  const encode = (value: unknown) => Buffer.from(JSON.stringify(value)).toString('base64url');
  return [
    encode({ alg: algorithm, typ: 'JWT' }),
    encode({
      iss: ISSUER,
      aud: CLIENT_ID,
      sub: 'operator-subject',
      iat: NOW_SECONDS,
      exp: NOW_SECONDS + 300,
      nonce,
    }),
    algorithm === 'none' ? '' : 'invalid-symmetric-signature',
  ].join('.');
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
