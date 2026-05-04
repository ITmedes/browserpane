import { describe, expect, it, vi } from 'vitest';
import { AuthConfigClient, AuthConfigMapper } from './auth-config';

describe('AuthConfigClient', () => {
  it('loads local compose OIDC metadata from auth-config.json', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse({
      mode: 'oidc',
      providerHint: 'keycloak',
      issuer: 'http://localhost:8091/realms/browserpane-dev',
      clientId: 'bpane-web',
      scope: 'openid',
      exampleUser: {
        username: 'demo',
        password: 'demo-demo',
      },
      mcpBridge: {
        controlUrl: 'http://localhost:8931/control-session',
        clientId: 'bpane-mcp-bridge',
        issuer: 'http://localhost:8091/realms/browserpane-dev',
        displayName: 'BrowserPane MCP bridge',
      },
    }));
    const client = new AuthConfigClient({
      baseUrl: 'http://localhost:8080/admin/',
      fetchImpl,
    });

    const config = await client.load();

    expect(config?.mode).toBe('oidc');
    expect(config?.exampleUser?.username).toBe('demo');
    expect(config?.mcpBridge?.clientId).toBe('bpane-mcp-bridge');
    expect(fetchImpl).toHaveBeenCalledWith(new URL('http://localhost:8080/auth-config.json'));
  });

  it('treats a missing auth config as unauthenticated local mode', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => new Response('', { status: 404 }));
    const client = new AuthConfigClient({
      baseUrl: 'http://localhost:8080',
      fetchImpl,
    });

    await expect(client.load()).resolves.toBeNull();
  });
});

describe('AuthConfigMapper', () => {
  it('rejects malformed auth config payloads at the boundary', () => {
    expect(() => AuthConfigMapper.toAuthConfig({ mode: '' })).toThrow(/mode/);
  });
});

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
