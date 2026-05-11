import { webcrypto } from 'node:crypto';
import { describe, expect, it, vi } from 'vitest';
import { BrowserTokenStore, type StorageLike } from './browser-token-store';
import { OidcAuthClient } from './oidc-auth-client';

const CONFIG = {
  mode: 'oidc',
  issuer: 'http://localhost:8091/realms/browserpane-dev',
  clientId: 'bpane-web',
  scope: 'openid',
  exampleUser: {
    username: 'demo',
    password: 'demo-demo',
  },
};

const METADATA = {
  authorization_endpoint: 'http://localhost:8091/realms/browserpane-dev/protocol/openid-connect/auth',
  token_endpoint: 'http://localhost:8091/realms/browserpane-dev/protocol/openid-connect/token',
  end_session_endpoint: 'http://localhost:8091/realms/browserpane-dev/protocol/openid-connect/logout',
};

describe('OidcAuthClient', () => {
  it('builds a PKCE authorization URL and stores login state', async () => {
    const storage = new MemoryStorage();
    const client = newClient(storage, jsonFetch(METADATA));

    const loginUrl = await client.buildLoginUrl(new URL('http://localhost:8080/admin/?x=1'));
    const url = new URL(loginUrl);

    expect(url.origin).toBe('http://localhost:8091');
    expect(url.searchParams.get('client_id')).toBe('bpane-web');
    expect(url.searchParams.get('redirect_uri')).toBe('http://localhost:8080/admin/?x=1');
    expect(url.searchParams.get('code_challenge_method')).toBe('S256');
    expect(storage.getItem('bpane.admin.auth.pkce.v1')).toContain('verifier');
  });

  it('exchanges an authorization code and stores an expiring token set', async () => {
    const storage = new MemoryStorage();
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/.well-known/openid-configuration')) {
        return jsonResponse(METADATA);
      }
      expect(init?.method).toBe('POST');
      expect(String(init?.body)).toContain('grant_type=authorization_code');
      return jsonResponse({
        access_token: jwt({ preferred_username: 'operator' }),
        id_token: jwt({ preferred_username: 'operator' }),
        refresh_token: 'refresh-token',
        expires_in: 300,
      });
    });
    const client = newClient(storage, fetchImpl, () => 1_000);
    const loginUrl = await client.buildLoginUrl(new URL('http://localhost:8080/admin/'));
    const returnedState = new URL(loginUrl).searchParams.get('state') ?? '';

    const completion = await client.completeLoginIfNeeded(
      new URL(`http://localhost:8080/admin/?code=abc&state=${returnedState}&session_state=kc`),
    );

    expect(completion).toEqual({ completed: true, cleanUrl: 'http://localhost:8080/admin/' });
    expect(client.getSnapshot()).toMatchObject({
      authenticated: true,
      username: 'operator',
    });
    expect(storage.getItem('bpane.admin.auth.tokens.v1')).toContain('refresh-token');
  });

  it('refreshes access tokens before expiry', async () => {
    const storage = new MemoryStorage();
    storage.setItem('bpane.admin.auth.tokens.v1', JSON.stringify({
      access_token: 'old-token',
      refresh_token: 'refresh-token',
      expiresAtMs: 50,
    }));
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/.well-known/openid-configuration')) {
        return jsonResponse(METADATA);
      }
      expect(String(init?.body)).toContain('grant_type=refresh_token');
      return jsonResponse({
        access_token: 'new-token',
        refresh_token: 'refresh-token',
        expires_in: 300,
      });
    });
    const client = newClient(storage, fetchImpl, () => 100);

    await expect(client.getValidAccessToken()).resolves.toBe('new-token');
  });

  it('builds a Keycloak logout URL and clears local token state', async () => {
    const storage = new MemoryStorage();
    storage.setItem('bpane.admin.auth.tokens.v1', JSON.stringify({
      access_token: 'access-token',
      id_token: 'id-token',
      expiresAtMs: 100_000,
    }));
    const client = newClient(storage, jsonFetch(METADATA));

    const logoutUrl = await client.buildLogoutUrl(new URL('http://localhost:8080/admin/?code=old'));

    expect(logoutUrl).toContain('/protocol/openid-connect/logout');
    expect(logoutUrl).toContain('post_logout_redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fadmin%2F');
    expect(storage.getItem('bpane.admin.auth.tokens.v1')).toBeNull();
  });
});

function newClient(
  storage: StorageLike,
  fetchImpl: typeof fetch,
  nowMs: () => number = () => Date.now(),
): OidcAuthClient {
  return new OidcAuthClient({
    config: CONFIG,
    tokenStore: new BrowserTokenStore(storage),
    fetchImpl,
    cryptoImpl: webcrypto as Crypto,
    nowMs,
  });
}

function jsonFetch(payload: unknown): typeof fetch {
  return vi.fn<typeof fetch>(async () => jsonResponse(payload));
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}

function jwt(payload: Record<string, unknown>): string {
  return `header.${btoa(JSON.stringify(payload)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '')}.sig`;
}

class MemoryStorage implements StorageLike {
  readonly #values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.#values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.#values.set(key, value);
  }

  removeItem(key: string): void {
    this.#values.delete(key);
  }
}
