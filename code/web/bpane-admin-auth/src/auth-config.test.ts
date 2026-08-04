import { describe, expect, it, vi } from 'vitest';
import { AuthConfigClient, AuthConfigMapper } from './auth-config';

const COMPLETE_CONFIG = {
  mode: 'oidc',
  providerHint: 'keycloak',
  issuer: 'http://localhost:8091/realms/browserpane-dev',
  clientId: 'bpane-web',
  scope: 'openid',
  exampleUser: { username: 'demo', password: 'demo-demo' },
  mcpBridge: {
    controlUrl: 'http://localhost:8080/api/v1/mcp-bridge/control-session',
    endpointBaseUrl: 'http://localhost:8931',
    clientId: 'bpane-mcp-bridge',
    issuer: 'http://localhost:8091/realms/browserpane-dev',
    displayName: 'BrowserPane MCP bridge',
  },
};

describe('AuthConfigClient', () => {
  it('loads and validates the complete runtime auth configuration', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse(COMPLETE_CONFIG));
    const client = new AuthConfigClient({
      baseUrl: 'http://localhost:8080/admin-new/projects',
      fetchImpl,
    });

    await expect(client.load()).resolves.toEqual(COMPLETE_CONFIG);
    expect(fetchImpl).toHaveBeenCalledWith(new URL('http://localhost:8080/auth-config.json'));
  });

  it('returns null for an intentionally absent configuration', async () => {
    const client = new AuthConfigClient({
      baseUrl: 'http://localhost:8080',
      fetchImpl: vi.fn<typeof fetch>(async () => new Response('', { status: 404 })),
    });

    await expect(client.load()).resolves.toBeNull();
  });

  it('rejects a failed configuration response', async () => {
    const client = new AuthConfigClient({
      baseUrl: 'http://localhost:8080',
      fetchImpl: vi.fn<typeof fetch>(async () => new Response('', { status: 503 })),
    });

    await expect(client.load()).rejects.toThrow('auth config request failed with HTTP 503');
  });
});

describe('AuthConfigMapper', () => {
  it('accepts the minimal disabled configuration', () => {
    expect(AuthConfigMapper.toAuthConfig({ mode: 'disabled' })).toEqual({ mode: 'disabled' });
  });

  it.each([
    [null, 'auth config'],
    [{ mode: '' }, 'mode'],
    [{ mode: 'oidc', exampleUser: {} }, 'username'],
    [{ mode: 'oidc', mcpBridge: { controlUrl: '' } }, 'controlUrl'],
    [{ mode: 'oidc', providerHint: 42 }, 'providerHint'],
  ])('rejects malformed payload %#', (payload, expectedMessage) => {
    expect(() => AuthConfigMapper.toAuthConfig(payload)).toThrow(expectedMessage);
  });
});

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
