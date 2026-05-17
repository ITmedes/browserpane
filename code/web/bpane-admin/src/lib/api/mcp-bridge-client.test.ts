import { describe, expect, it, vi } from 'vitest';
import { McpBridgeClient } from './mcp-bridge-client';

describe('McpBridgeClient', () => {
  it('loads bridge health from the control URL origin', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse({
      status: 'ok',
      clients: 0,
      control_session_id: 'session-a',
      control_session_state: 'active',
      control_session_backend_delegated: true,
      bridge_alignment: 'aligned',
      managed_sessions: [{
        kind: 'control',
        session_id: 'session-a',
        clients: 1,
        state: 'active',
        mode: 'session_runtime_pool',
        visible: true,
        backend_delegated: true,
        mcp_owner: true,
        cdp_endpoint: 'http://runtime:9223',
        playwright_cdp_endpoint: 'http://runtime:9223',
        playwright_effective_cdp_endpoint: 'http://runtime:9223',
        alignment: 'aligned',
      }],
    }));
    const client = new McpBridgeClient({
      controlUrl: 'http://localhost:8931/control-session',
      fetchImpl,
    });

    const health = await client.getHealth();

    expect(health.control_session_id).toBe('session-a');
    expect(health.bridge_alignment).toBe('aligned');
    expect(health.managed_sessions[0]?.mcp_owner).toBe(true);
    expect(health.managed_sessions[0]?.clients).toBe(1);
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8931/health'),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('preserves path prefixes when loading bridge health', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse({
      status: 'ok',
      clients: 0,
      control_session_id: null,
      control_session_state: null,
      control_session_backend_delegated: false,
      bridge_alignment: null,
      managed_sessions: [],
    }));
    const client = new McpBridgeClient({
      controlUrl: 'https://example.test/mcp-control/control-session',
      fetchImpl,
    });

    await client.getHealth();

    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('https://example.test/mcp-control/health'),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('sets and clears the bridge control session', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse({
      session: { id: 'session-b' },
      cdp_endpoint: 'http://runtime:9223',
    }));
    const client = new McpBridgeClient({
      controlUrl: 'http://localhost:8931/control-session',
      fetchImpl,
    });

    const response = await client.setControlSession('session-b');
    await client.clearControlSession();

    expect(response.session_id).toBe('session-b');
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL('http://localhost:8931/control-session'),
      expect.objectContaining({
        method: 'PUT',
        body: JSON.stringify({ session_id: 'session-b' }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      new URL('http://localhost:8931/control-session'),
      expect.objectContaining({ method: 'DELETE' }),
    );
  });
});

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
