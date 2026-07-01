import { describe, expect, it, vi } from 'vitest';

import { McpBridgeClient, toMcpBridgeHealth } from './mcp-bridge-client';

describe('McpBridgeClient', () => {
  it('loads bridge health from the health endpoint beside control-session', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      expect(String(input)).toBe('http://localhost:8931/health');
      return jsonResponse({
        status: 'ok',
        clients: 1,
        control_session_id: 'session-1',
        control_session_state: 'ready',
        control_session_backend_delegated: true,
        bridge_alignment: 'aligned',
        managed_sessions: [{
          kind: 'control',
          session_id: 'session-1',
          clients: 1,
          state: 'ready',
          mode: 'docker_pool',
          visible: true,
          backend_delegated: true,
          mcp_owner: true,
          cdp_endpoint: 'http://browser:9222',
          playwright_cdp_endpoint: null,
          playwright_effective_cdp_endpoint: 'http://browser:9222',
          alignment: 'aligned',
        }],
      }, 200);
    });
    const client = new McpBridgeClient({
      controlUrl: 'http://localhost:8931/control-session',
      fetchImpl,
    });

    const health = await client.getHealth();

    expect(health.control_session_id).toBe('session-1');
    expect(health.managed_sessions[0]).toMatchObject({
      session_id: 'session-1',
      clients: 1,
      mcp_owner: true,
    });
  });

  it('sets and clears the bridge default session', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      if (init?.method === 'PUT') {
        expect(String(input)).toBe('https://example.test/mcp/control-session');
        expect(JSON.parse(String(init.body))).toEqual({ session_id: 'session-2' });
        return jsonResponse({ session: { id: 'session-2' }, cdp_endpoint: 'http://browser:9222' }, 200);
      }
      if (init?.method === 'DELETE') {
        return new Response(null, { status: 204 });
      }
      return new Response('not found', { status: 404 });
    });
    const client = new McpBridgeClient({
      controlUrl: 'https://example.test/mcp/control-session',
      fetchImpl,
    });

    const control = await client.setControlSession('session-2');
    await client.clearControlSession();

    expect(control.session_id).toBe('session-2');
    expect(fetchImpl.mock.calls.map((call) => call[1]?.method)).toEqual(['PUT', 'DELETE']);
  });

  it('rejects malformed health payloads', () => {
    expect(() => toMcpBridgeHealth({ status: 'ok', managed_sessions: {} })).toThrow(
      'managed_sessions must be an array',
    );
  });
});

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}
