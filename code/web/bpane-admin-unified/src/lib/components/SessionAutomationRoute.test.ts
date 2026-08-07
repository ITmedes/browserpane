import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload } from '$lib/test-utils/session-fixtures';
import { workflowRunFixture } from '$lib/test-utils/workflow-run-fixture';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionAutomationRoute from './SessionAutomationRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('SessionAutomationRoute', () => {
  it('loads only workflow runs associated with the selected session', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/workflow-runs')) {
        return jsonResponse({
          runs: [
            workflowRunFixture({ id: 'run-session-1', session_id: 'session-1' }),
            workflowRunFixture({ id: 'run-session-2', session_id: 'session-2' }),
          ],
        });
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload());
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionAutomationRoute, {
      authContext: authContext(),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-automation-total').textContent).toContain('1');
    });
    expect(byTestId(target, 'session-subarea-automation').getAttribute('aria-current')).toBe('page');
    expect(byTestId(target, 'session-workflow-run-run-session-1')).toBeTruthy();
    expect(target.querySelector('[data-testid="session-workflow-run-run-session-2"]')).toBeNull();
    expect(byTestId(target, 'session-workflow-run-open-run-session-1').getAttribute('href'))
      .toBe('/admin-new/workflow-runs/run-session-1');
    expect(byTestId(target, 'mcp-delegation-status').textContent).toContain('Not configured');
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
        return jsonResponse(bridgeHealth({ delegated, defaultSessionId }));
      }
      if (url === 'http://localhost:3000/api/v1/mcp-bridge/control-session' && init?.method === 'PUT') {
        expect(new Headers(init?.headers).get('authorization')).toBe('Bearer shell-token');
        defaultSessionId = JSON.parse(String(init.body)).session_id;
        return jsonResponse({ session: { id: defaultSessionId }, cdp_endpoint: 'http://browser:9222' });
      }
      if (url === 'http://localhost:3000/api/v1/mcp-bridge/control-session' && init?.method === 'DELETE') {
        expect(new Headers(init?.headers).get('authorization')).toBe('Bearer shell-token');
        defaultSessionId = null;
        return new Response(null, { status: 204 });
      }
      if (url.endsWith('/api/v1/workflow-runs')) {
        return jsonResponse({ runs: [] });
      }
      if (url.endsWith('/api/v1/sessions/session-1/automation-owner') && init?.method === 'POST') {
        delegated = true;
        return jsonResponse(sessionPayload({ automationDelegate: mcpDelegate() }));
      }
      if (url.endsWith('/api/v1/sessions/session-1/automation-owner') && init?.method === 'DELETE') {
        delegated = false;
        return jsonResponse(sessionPayload({ automationDelegate: null }));
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({
          automationDelegate: delegated ? mcpDelegate() : null,
        }));
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionAutomationRoute, {
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
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'mcp-delegation-status').textContent).toContain('Not authorized');
    });
    expect(byTestId(target, 'mcp-endpoint-url').textContent).toContain('/sessions/session-1/mcp');

    byTestId(target, 'mcp-authorize').click();
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

  it('surfaces authenticated route failures through the shared handler', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(SessionAutomationRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-automation-error').textContent).toContain('unavailable');
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

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
