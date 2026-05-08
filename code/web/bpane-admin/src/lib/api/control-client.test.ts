import { describe, expect, it, vi } from 'vitest';
import { ControlApiError, ControlClient, type FetchLike } from './control-client';

const SESSION = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  state: 'active',
  owner_mode: 'shared',
  connect: {
    gateway_url: 'https://localhost:4433',
    transport_path: '/session',
    auth_type: 'session_connect_ticket',
    ticket_path: '/api/v1/sessions/019df4d2-f4f7-7b00-9e0c-79683b1c82f6/access-tokens',
    compatibility_mode: 'session_runtime_pool',
  },
  runtime: {
    binding: 'docker_runtime_pool',
    compatibility_mode: 'session_runtime_pool',
    cdp_endpoint: 'http://runtime:9223',
  },
  status: {
    runtime_state: 'running',
    presence_state: 'connected',
    connection_counts: {
      interactive_clients: 1,
      owner_clients: 1,
      viewer_clients: 0,
      recorder_clients: 0,
      automation_clients: 0,
      total_clients: 1,
    },
    stop_eligibility: {
      allowed: false,
      blockers: [{ kind: 'owner_clients', count: 1 }],
    },
  },
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
  stopped_at: null,
};

describe('ControlClient', () => {
  it('lists owner-visible sessions with bearer auth', async () => {
    const fetchImpl = jsonFetch({ sessions: [SESSION] });
    const client = new ControlClient({
      baseUrl: 'https://browserpane.example/app/',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const response = await client.listSessions();

    expect(response.sessions).toHaveLength(1);
    expect(response.sessions[0]?.id).toBe(SESSION.id);
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('https://browserpane.example/api/v1/sessions'),
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({
          accept: 'application/json',
          authorization: 'Bearer owner-token',
        }),
      }),
    );
  });

  it('creates sessions through the frozen v1 endpoint', async () => {
    const fetchImpl = jsonFetch(SESSION);
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    await client.createSession({ idle_timeout_sec: 300, labels: { source: 'admin-smoke' } });

    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/sessions'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ idle_timeout_sec: 300, labels: { source: 'admin-smoke' } }),
        headers: expect.objectContaining({
          'content-type': 'application/json',
        }),
      }),
    );
  });

  it('encodes session ids for lifecycle operations', async () => {
    const fetchImpl = jsonFetch(SESSION);
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    await client.stopSession('session/with/slash');

    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/sessions/session%2Fwith%2Fslash/stop'),
      expect.objectContaining({ method: 'POST' }),
    );
  });

  it('issues session-scoped connect tickets', async () => {
    const fetchImpl = jsonFetch({
      session_id: SESSION.id,
      token_type: 'session_connect_ticket',
      token: 'connect-ticket',
      expires_at: '2026-05-04T19:05:00Z',
      connect: SESSION.connect,
    });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const response = await client.issueSessionAccessToken(SESSION.id);

    expect(response.token).toBe('connect-ticket');
    expect(response.connect.auth_type).toBe('session_connect_ticket');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL(`http://localhost:8932/api/v1/sessions/${SESSION.id}/access-tokens`),
      expect.objectContaining({ method: 'POST' }),
    );
  });

  it('sets and clears a session automation delegate', async () => {
    const fetchImpl = jsonFetch({
      ...SESSION,
      automation_delegate: {
        client_id: 'bpane-mcp-bridge',
        issuer: 'http://localhost:8091/realms/bpane',
        display_name: 'BrowserPane MCP bridge',
      },
    });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const delegated = await client.setAutomationDelegate(SESSION.id, {
      client_id: 'bpane-mcp-bridge',
      issuer: 'http://localhost:8091/realms/bpane',
      display_name: 'BrowserPane MCP bridge',
    });
    await client.clearAutomationDelegate(SESSION.id);

    expect(delegated.automation_delegate?.client_id).toBe('bpane-mcp-bridge');
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL(`http://localhost:8932/api/v1/sessions/${SESSION.id}/automation-owner`),
      expect.objectContaining({ method: 'POST' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      new URL(`http://localhost:8932/api/v1/sessions/${SESSION.id}/automation-owner`),
      expect.objectContaining({ method: 'DELETE' }),
    );
  });

  it('throws a typed API error for non-success responses', async () => {
    const fetchImpl = vi.fn<FetchLike>(async () => new Response('denied', { status: 403 }));
    const onAuthenticationFailure = vi.fn();
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      onAuthenticationFailure,
      fetchImpl,
    });

    await expect(client.listSessions()).rejects.toMatchObject({ status: 403, body: 'denied' });
    expect(onAuthenticationFailure).not.toHaveBeenCalled();
  });

  it('notifies the app about expired owner bearer auth', async () => {
    const fetchImpl = vi.fn<FetchLike>(async () => new Response('expired', { status: 401 }));
    const onAuthenticationFailure = vi.fn();
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      onAuthenticationFailure,
      fetchImpl,
    });

    await expect(client.listSessions()).rejects.toMatchObject({ status: 401 });
    expect(onAuthenticationFailure).toHaveBeenCalledWith(expect.objectContaining({ status: 401, body: 'expired' }));
  });
});

function jsonFetch(payload: unknown): ReturnType<typeof vi.fn<FetchLike>> {
  return vi.fn<FetchLike>(async () => new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  }));
}
