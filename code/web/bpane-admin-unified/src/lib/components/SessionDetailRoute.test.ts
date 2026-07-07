import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload, sessionStatusPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionDetailRoute from './SessionDetailRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/sessions/session-1');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('SessionDetailRoute', () => {
  it('loads session detail and disconnects clients through the authenticated API', async () => {
    let clientsConnected = true;
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload({ totalClients: clientsConnected ? 1 : 0 }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1/connections/disconnect-all') && init?.method === 'POST') {
        clientsConnected = false;
        return jsonResponse(sessionStatusPayload({ totalClients: 0 }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: clientsConnected ? 1 : 0 }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-title').textContent).toContain('session-1');
    });
    expect(byTestId(target, 'session-detail-total-clients').textContent).toContain('1');
    byTestId(target, 'session-disconnect-all').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-action-success').textContent).toContain('disconnected');
    });
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-total-clients').textContent).toContain('0');
    });
    const disconnectCall = fetchImpl.mock.calls.find((call) => String(call[0]).endsWith('/connections/disconnect-all'));
    const headers = disconnectCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('opens the selected session in a popup preview window', async () => {
    const focus = vi.fn();
    const open = vi.spyOn(window, 'open').mockReturnValue({ focus } as unknown as Window);
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload({ totalClients: 0 }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: 0 }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-title').textContent).toContain('session-1');
    });
    byTestId(target, 'session-connect-preview').click();

    expect(open).toHaveBeenCalledWith(
      '/admin-new/sessions/session-1/preview',
      'bpane-session-preview-session-1',
      'popup=yes,width=1440,height=960,resizable=yes,scrollbars=no',
    );
    expect(focus).toHaveBeenCalledOnce();
  });

  it('updates recording policy for the selected session', async () => {
    let recordingMode = 'disabled';
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload({ totalClients: 0 }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1/recording-policy') && init?.method === 'PUT') {
        recordingMode = JSON.parse(String(init.body)).mode;
        return jsonResponse(sessionPayload({ totalClients: 0, recordingMode }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: 0, recordingMode }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-recording').textContent).toContain('disabled');
    });

    byTestId(target, 'session-enable-recording').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-recording').textContent).toContain('always / webm');
    });
    expect(byTestId(target, 'session-detail-action-success').textContent).toContain('enabled');

    byTestId(target, 'session-disable-recording').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-recording').textContent).toContain('disabled');
    });

    const recordingPolicyCalls = fetchImpl.mock.calls
      .filter((call) => String(call[0]).endsWith('/recording-policy'));
    expect(recordingPolicyCalls.map((call) => [call[1]?.method, JSON.parse(String(call[1]?.body)).mode])).toEqual([
      ['PUT', 'always'],
      ['PUT', 'disabled'],
    ]);
  });

  it('authorizes MCP, manages bridge default state, copies the endpoint, and revokes authorization', async () => {
    let delegated = false;
    let defaultSessionId: string | null = null;
    const clipboardWrite = vi.fn<Clipboard['writeText']>();
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: clipboardWrite },
    });
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url === 'http://localhost:3000/api/v1/mcp-bridge/health') {
        expect(new Headers(init?.headers).get('authorization')).toBe('Bearer shell-token');
        return jsonResponse(bridgeHealth({ delegated, defaultSessionId }), 200);
      }
      if (url === 'http://localhost:3000/api/v1/mcp-bridge/control-session' && init?.method === 'PUT') {
        expect(new Headers(init?.headers).get('authorization')).toBe('Bearer shell-token');
        defaultSessionId = JSON.parse(String(init.body)).session_id;
        return jsonResponse({ session: { id: defaultSessionId }, cdp_endpoint: 'http://browser:9222' }, 200);
      }
      if (url === 'http://localhost:3000/api/v1/mcp-bridge/control-session' && init?.method === 'DELETE') {
        expect(new Headers(init?.headers).get('authorization')).toBe('Bearer shell-token');
        defaultSessionId = null;
        return new Response(null, { status: 204 });
      }
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload({ totalClients: 0 }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1/automation-owner') && init?.method === 'POST') {
        delegated = true;
        return jsonResponse(sessionPayload({ totalClients: 0, automationDelegate: mcpDelegate() }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1/automation-owner') && init?.method === 'DELETE') {
        delegated = false;
        return jsonResponse(sessionPayload({ totalClients: 0, automationDelegate: null }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({
          totalClients: 0,
          automationDelegate: delegated ? mcpDelegate() : null,
        }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionDetailRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'shell-token',
        authConfig: {
          mode: 'oidc',
          mcpBridge: {
            controlUrl: 'http://localhost:3000/api/v1/mcp-bridge/control-session',
            endpointBaseUrl: 'http://localhost:8931',
            clientId: 'bpane-mcp-bridge',
            issuer: 'http://localhost:8091/realms/browserpane',
            displayName: 'Local MCP bridge',
          },
        },
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'mcp-delegation-status').textContent).toContain('Not authorized');
    });
    expect(byTestId(target, 'mcp-endpoint-url').textContent).toContain('/sessions/session-1/mcp');

    byTestId(target, 'mcp-authorize').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'mcp-action-success').textContent).toContain('authorized');
    });
    await vi.waitFor(() => {
      expect(byTestId(target, 'mcp-delegation-status').textContent).toContain('Authorized');
    });

    byTestId(target, 'mcp-set-default').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'mcp-delegation-status').textContent).toContain('Authorized default');
    });

    byTestId(target, 'mcp-copy-endpoint').click();
    await vi.waitFor(() => {
      expect(clipboardWrite).toHaveBeenCalledWith('http://localhost:8931/sessions/session-1/mcp');
    });

    byTestId(target, 'mcp-clear-default').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'mcp-default-session').textContent).toContain('No default');
    });
    byTestId(target, 'mcp-revoke').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'mcp-delegation-status').textContent).toContain('Not authorized');
    });

    const calls = fetchImpl.mock.calls.map((call) => [String(call[0]), call[1]?.method ?? 'GET']);
    expect(calls).toContainEqual(['http://localhost:3000/api/v1/sessions/session-1/automation-owner', 'POST']);
    expect(calls).toContainEqual(['http://localhost:3000/api/v1/mcp-bridge/control-session', 'PUT']);
    expect(calls).toContainEqual(['http://localhost:3000/api/v1/mcp-bridge/control-session', 'DELETE']);
    expect(calls).toContainEqual(['http://localhost:3000/api/v1/sessions/session-1/automation-owner', 'DELETE']);
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(SessionDetailRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-error').textContent).toContain('Session detail unavailable');
    });
    expect(onAuthenticationFailure).toHaveBeenCalled();
  });
});

function authContext(
  overrides: Partial<Pick<UnifiedAdminContext, 'accessTokenProvider' | 'onAuthenticationFailure' | 'authConfig'>> = {},
): UnifiedAdminContext {
  return {
    auth: {
      configured: true,
      authenticated: true,
      username: 'demo',
      accessToken: 'token',
      claims: null,
    },
    authConfig: overrides.authConfig ?? null,
    accessTokenProvider: overrides.accessTokenProvider ?? (async () => 'token'),
    onAuthenticationFailure: overrides.onAuthenticationFailure ?? vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}

function mcpDelegate(): Record<string, unknown> {
  return {
    client_id: 'bpane-mcp-bridge',
    issuer: 'http://localhost:8091/realms/browserpane',
    display_name: 'Local MCP bridge',
  };
}

function bridgeHealth(input: {
  readonly delegated: boolean;
  readonly defaultSessionId: string | null;
}): Record<string, unknown> {
  return {
    status: 'ok',
    clients: 0,
    control_session_id: input.defaultSessionId,
    control_session_state: input.defaultSessionId ? 'ready' : null,
    control_session_backend_delegated: Boolean(input.defaultSessionId && input.delegated),
    bridge_alignment: input.defaultSessionId ? 'aligned' : 'unmanaged',
    managed_sessions: input.defaultSessionId
      ? [{
          kind: 'control',
          session_id: input.defaultSessionId,
          clients: 0,
          state: 'ready',
          mode: 'docker_pool',
          visible: true,
          backend_delegated: input.delegated,
          mcp_owner: false,
          cdp_endpoint: 'http://browser:9222',
          playwright_cdp_endpoint: null,
          playwright_effective_cdp_endpoint: null,
          alignment: 'aligned',
        }]
      : [],
  };
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}
