import { describe, expect, it, vi } from 'vitest';

import {
  SessionCatalogClient,
  SessionCatalogError,
  toSessionListResponse,
} from './session-client';
import { sessionPayload, sessionStatusPayload } from '$lib/test-utils/session-fixtures';

describe('SessionCatalogClient', () => {
  it('lists, creates, and loads session status through authenticated endpoints', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions') && init?.method === 'GET') {
        return jsonResponse({ sessions: [sessionPayload()] }, 200);
      }
      if (url.endsWith('/api/v1/sessions') && init?.method === 'POST') {
        return jsonResponse(sessionPayload({ id: 'created-session', totalClients: 0 }), 201);
      }
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload(), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1/access-tokens') && init?.method === 'POST') {
        return jsonResponse(sessionAccessTokenPayload(), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1/recording-policy') && init?.method === 'PUT') {
        expect(JSON.parse(String(init.body))).toEqual({
          mode: 'manual',
          format: 'webm',
          retention_sec: 3600,
        });
        return jsonResponse(sessionPayload({ recordingMode: 'manual', recordingRetentionSec: 3600 }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload(), 200);
      }
      return new Response('not found', { status: 404 });
    });
    const client = new SessionCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const listed = await client.listSessions();
    const created = await client.createSession({ labels: { bpane_admin_surface: 'unified' } });
    const loaded = await client.getSession('session-1');
    const status = await client.getSessionStatus('session-1');
    const access = await client.issueSessionAccessToken('session-1');
    const recording = await client.updateSessionRecordingPolicy('session-1', {
      mode: 'manual',
      format: 'webm',
      retention_sec: 3600,
    });

    expect(listed.sessions[0]?.id).toBe('session-1');
    expect(listed.sessions[0]?.recording).toMatchObject({
      mode: 'disabled',
      format: 'webm',
    });
    expect(created.id).toBe('created-session');
    expect(loaded.runtime.binding).toBe('docker:browser-1');
    expect(status.connections[0]).toMatchObject({ connection_id: 7, role: 'browser-owner' });
    expect(status.recording).toMatchObject({
      configured_mode: 'always',
      state: 'idle',
      active_recording_id: null,
    });
    expect(access).toMatchObject({
      session_id: 'session-1',
      token_type: 'session_connect_ticket',
      token: 'connect-ticket',
    });
    expect(recording.recording).toMatchObject({
      mode: 'manual',
      retention_sec: 3600,
    });
    const createBody = JSON.parse(String(fetchImpl.mock.calls[1]?.[1]?.body));
    expect(createBody.labels).toEqual({ bpane_admin_surface: 'unified' });
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer token-1');
  });

  it('maps lifecycle mutations to the control API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/connections/disconnect-all')) {
        return jsonResponse(sessionStatusPayload({ totalClients: 0 }), 200);
      }
      return jsonResponse(sessionPayload({ totalClients: 0 }), 200);
    });
    const client = new SessionCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    await client.cancelQueuedSession('session-1');
    await client.releaseSessionRuntime('session-1');
    await client.stopSession('session-1');
    await client.killSession('session-1');
    await client.disconnectAllSessionConnections('session-1');

    expect(fetchImpl.mock.calls.map((call) => [call[0].toString(), call[1]?.method])).toEqual([
      ['http://browserpane.test/api/v1/sessions/session-1/cancel', 'POST'],
      ['http://browserpane.test/api/v1/sessions/session-1/release', 'POST'],
      ['http://browserpane.test/api/v1/sessions/session-1/stop', 'POST'],
      ['http://browserpane.test/api/v1/sessions/session-1/kill', 'POST'],
      ['http://browserpane.test/api/v1/sessions/session-1/connections/disconnect-all', 'POST'],
    ]);
  });

  it('sets and clears the session automation delegate through authenticated endpoints', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/automation-owner') && init?.method === 'POST') {
        expect(JSON.parse(String(init.body))).toEqual({
          client_id: 'bpane-mcp-bridge',
          issuer: 'http://issuer.test',
          display_name: 'MCP bridge',
        });
        return jsonResponse(sessionPayload({
          automationDelegate: {
            client_id: 'bpane-mcp-bridge',
            issuer: 'http://issuer.test',
            display_name: 'MCP bridge',
          },
        }), 200);
      }
      if (url.endsWith('/automation-owner') && init?.method === 'DELETE') {
        return jsonResponse(sessionPayload({ automationDelegate: null }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    const client = new SessionCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const delegated = await client.setAutomationDelegate('session-1', {
      client_id: 'bpane-mcp-bridge',
      issuer: 'http://issuer.test',
      display_name: 'MCP bridge',
    });
    const cleared = await client.clearAutomationDelegate('session-1');

    expect(delegated.automation_delegate?.client_id).toBe('bpane-mcp-bridge');
    expect(cleared.automation_delegate).toBeNull();
    expect(fetchImpl.mock.calls.map((call) => [call[0].toString(), call[1]?.method])).toEqual([
      ['http://browserpane.test/api/v1/sessions/session-1/automation-owner', 'POST'],
      ['http://browserpane.test/api/v1/sessions/session-1/automation-owner', 'DELETE'],
    ]);
  });

  it('delegates authentication failures and rejects missing tokens', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new SessionCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'expired',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure,
    });

    await expect(client.listSessions()).rejects.toMatchObject({
      code: 'http_error',
      status: 401,
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();

    const missingTokenClient = new SessionCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => null,
      fetchImpl: vi.fn(),
    });
    await expect(missingTokenClient.listSessions()).rejects.toMatchObject({ code: 'missing_token' });
  });
});

describe('toSessionListResponse', () => {
  it('maps known payloads and rejects invalid session lists', () => {
    const mapped = toSessionListResponse({ sessions: [sessionPayload()] });

    expect(mapped.sessions[0]).toMatchObject({
      id: 'session-1',
      browser_context: { mode: 'reusable', context_id: 'context-1' },
      capabilities: { browser_input: true, file_transfer: true },
    });
    expect(() => toSessionListResponse({ sessions: {} })).toThrow(SessionCatalogError);
  });
});

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function sessionAccessTokenPayload() {
  return {
    session_id: 'session-1',
    token_type: 'session_connect_ticket',
    token: 'connect-ticket',
    expires_at: '2026-06-21T10:10:00.000Z',
    connect: {
      gateway_url: 'https://localhost:4433',
      transport_path: '/session/session-1',
      auth_type: 'session_connect_ticket',
      ticket_path: '/api/v1/sessions/session-1/access-tokens',
      compatibility_mode: 'webtransport',
    },
  };
}
