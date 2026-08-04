import { webcrypto } from 'node:crypto';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import type { AuthConfig } from './auth-config';
import { BrowserTokenStore } from './browser-token-store';
import { OidcAuthClientFactory } from './oidc-auth-client-factory';
import { MemoryStorage } from './test-support/memory-storage';
import { OidcTokenFixture } from './test-support/oidc-token-fixture';

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
};

describe('OidcAuthClient', () => {
  let fixture: OidcTokenFixture;

  beforeAll(async () => {
    fixture = await OidcTokenFixture.create(ISSUER, CLIENT_ID);
  });

  it('completes a nonce-bound login and persists no refresh token', async () => {
    const storage = new MemoryStorage();
    let tokenResponse: unknown = null;
    const client = newClient(storage, async (input, init) => {
      if (String(input).endsWith('/token')) {
        expect(init?.method).toBe('POST');
        return jsonResponse(tokenResponse);
      }
      return metadataOrKeys(input, fixture);
    });
    const loginUrl = new URL(await client.buildLoginUrl(new URL('http://localhost:8080/admin-new/projects')));
    const nonce = loginUrl.searchParams.get('nonce') ?? '';
    tokenResponse = {
      access_token: 'access-token',
      id_token: await fixture.sign(NOW_SECONDS, { nonce, preferred_username: 'operator' }),
      refresh_token: 'memory-only-refresh-token',
      expires_in: 300,
    };

    const completion = await client.completeLoginIfNeeded(new URL(
      `http://localhost:8080/admin-new/projects?code=code&state=${loginUrl.searchParams.get('state')}`,
    ));

    expect(completion.completed).toBe(true);
    expect(client.getSnapshot()).toMatchObject({ authenticated: true, username: 'operator' });
    expect(storage.getItem('bpane.admin.auth.tokens.v2')).not.toContain('memory-only-refresh-token');
  });

  it('reverifies a persisted ID token before restoring authentication', async () => {
    const storage = new MemoryStorage();
    const idToken = await fixture.sign(NOW_SECONDS, { preferred_username: 'restored-operator' });
    storage.setItem('bpane.admin.auth.tokens.v2', JSON.stringify({
      access_token: 'stored-access-token',
      id_token: idToken,
      expiresAtMs: (NOW_SECONDS + 300) * 1000,
    }));
    const client = newClient(storage, (input) => metadataOrKeys(input, fixture));

    expect(client.getSnapshot().authenticated).toBe(false);
    await client.initialize();
    expect(client.getSnapshot()).toMatchObject({ authenticated: true, username: 'restored-operator' });
  });

  it('clears persisted authentication when restored identity verification fails', async () => {
    const storage = new MemoryStorage();
    storage.setItem('bpane.admin.auth.tokens.v2', JSON.stringify({
      access_token: 'stored-access-token',
      id_token: await fixture.sign(NOW_SECONDS, { aud: 'wrong-client' }),
      expiresAtMs: (NOW_SECONDS + 300) * 1000,
    }));
    const client = newClient(storage, (input) => metadataOrKeys(input, fixture));

    await client.initialize();

    expect(client.getSnapshot().authenticated).toBe(false);
    expect(storage.getItem('bpane.admin.auth.tokens.v2')).toBeNull();
  });

  it('rejects a callback with the wrong state before token exchange', async () => {
    const storage = new MemoryStorage();
    const fetchImpl = vi.fn<typeof fetch>((input) => metadataOrKeys(input, fixture));
    const client = newClient(storage, fetchImpl);
    await client.buildLoginUrl(new URL('http://localhost:8080/admin/'));

    await expect(client.completeLoginIfNeeded(new URL(
      'http://localhost:8080/admin/?code=code&state=wrong',
    ))).rejects.toThrow('OIDC state mismatch');
    expect(fetchImpl).toHaveBeenCalledTimes(1);
  });

  it('clears state when refresh fails and returns no credential', async () => {
    const storage = new MemoryStorage();
    let nowMs = NOW_SECONDS * 1000;
    let tokenResponse: unknown = null;
    const client = newClient(storage, async (input) => {
      if (String(input).endsWith('/token')) {
        return tokenResponse ? jsonResponse(tokenResponse) : new Response('', { status: 401 });
      }
      return metadataOrKeys(input, fixture);
    }, () => nowMs);
    const login = new URL(await client.buildLoginUrl(new URL('http://localhost:8080/admin/')));
    tokenResponse = {
      access_token: 'access-token',
      id_token: await fixture.sign(NOW_SECONDS, { nonce: login.searchParams.get('nonce') }),
      refresh_token: 'refresh-token',
      expires_in: 61,
    };
    await client.completeLoginIfNeeded(new URL(
      `http://localhost:8080/admin/?code=code&state=${login.searchParams.get('state')}`,
    ));
    tokenResponse = null;
    nowMs += 2_000;

    await expect(client.getValidAccessToken()).resolves.toBeNull();
    expect(client.getSnapshot().authenticated).toBe(false);
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
    cryptoImpl: webcrypto as Crypto,
    nowMs,
  });
}

function metadataOrKeys(input: RequestInfo | URL, fixture: OidcTokenFixture): Promise<Response> {
  return Promise.resolve(String(input).endsWith('/jwks')
    ? jsonResponse(fixture.keySet())
    : jsonResponse(METADATA));
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
