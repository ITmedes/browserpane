import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { egressDiagnosticsPayload } from '$lib/test-utils/egress-fixtures';
import { sessionPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionNetworkRoute from './SessionNetworkRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('SessionNetworkRoute', () => {
  it('loads sanitized network evidence and runs an active probe for a ready runtime', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/egress-diagnostics') && init?.method === 'POST') {
        expect(JSON.parse(String(init.body))).toEqual({});
        return jsonResponse(egressDiagnosticsPayload({
          proof_level: 'active_probe',
          proof: {
            ...egressDiagnosticsPayload().proof as Record<string, unknown>,
            active_probe_collected: true,
            observed_public_ip: '203.0.113.10',
            observed_tls_issuer: 'Example Trust Services',
          },
        }));
      }
      if (url.endsWith('/egress-diagnostics')) {
        return jsonResponse(egressDiagnosticsPayload());
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ state: 'ready', runtimeState: 'running' }));
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionNetworkRoute, {
      authContext: authContext(),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-network-profile-label').textContent).toContain('Support proxy');
    });
    expect(byTestId(target, 'session-subarea-network').getAttribute('aria-current')).toBe('page');
    expect(byTestId(target, 'session-network-profile-link').getAttribute('href'))
      .toBe('/admin-new/egress/egress-1');
    expect(byTestId(target, 'session-network-requested-timezone').textContent).toContain('UTC');
    expect(byTestId(target, 'session-network-runtime-binding').textContent).toContain('docker:browser-1');
    expect((byTestId(target, 'session-network-probe') as HTMLButtonElement).disabled).toBe(false);

    byTestId(target, 'session-network-probe').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-network-action-success').textContent).toContain('evidence collected');
    });
    expect(byTestId(target, 'session-network-public-ip').textContent).toContain('203.0.113.10');
    expect(byTestId(target, 'session-network-tls-issuer').textContent).toContain('Example Trust Services');
  });

  it('does not offer an active probe for a stopped runtime', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/egress-diagnostics')) {
        return jsonResponse(egressDiagnosticsPayload({ runtime_assignment: null }));
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ state: 'stopped', runtimeState: 'stopped', totalClients: 0 }));
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionNetworkRoute, { authContext: authContext(), sessionId: 'session-1' });

    await vi.waitFor(() => expect(byTestId(target, 'session-network-probe-blocked')).toBeTruthy());
    expect((byTestId(target, 'session-network-probe') as HTMLButtonElement).disabled).toBe(true);
    byTestId(target, 'session-network-probe').click();
    expect(fetchImpl.mock.calls.filter((call) => call[1]?.method === 'POST')).toHaveLength(0);
  });

  it('preserves requested and effective identity when diagnostics loading fails', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/egress-diagnostics')) {
        return new Response('diagnostics unavailable', { status: 503 });
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload());
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionNetworkRoute, { authContext: authContext(), sessionId: 'session-1' });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-network-diagnostics-error').textContent).toContain('HTTP 503');
    });
    expect(byTestId(target, 'session-network-requested-profile').textContent).toContain('egress-1');
    expect(byTestId(target, 'session-network-effective-profile').textContent).toContain('Support proxy');
  });

  it('renders persisted failure evidence when a probe completes without observations', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/egress-diagnostics') && init?.method === 'POST') {
        return jsonResponse(egressDiagnosticsPayload({
          health: 'attention',
          proof: {
            ...egressDiagnosticsPayload().proof as Record<string, unknown>,
            active_probe_collected: false,
            last_failure_reason: 'Browser probe timed out.',
          },
        }));
      }
      if (url.endsWith('/egress-diagnostics')) {
        return jsonResponse(egressDiagnosticsPayload());
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ state: 'ready', runtimeState: 'running' }));
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionNetworkRoute, { authContext: authContext(), sessionId: 'session-1' });

    await vi.waitFor(() => expect(byTestId(target, 'session-network-probe')).toBeTruthy());
    byTestId(target, 'session-network-probe').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-network-action-error').textContent).toContain('Browser probe timed out');
    });
    expect(byTestId(target, 'session-network-warnings').textContent).toContain('Browser probe timed out');
  });

  it('delegates fatal session authentication failures to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(SessionNetworkRoute, {
      authContext: authContext({ onAuthenticationFailure }),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-network-error').textContent).toContain('HTTP 401');
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });
});

function authContext(
  overrides: Partial<Pick<UnifiedAdminContext, 'onAuthenticationFailure'>> = {},
): UnifiedAdminContext {
  return {
    auth: { configured: true, authenticated: true, username: 'demo', accessToken: 'token', claims: null },
    authConfig: null,
    accessTokenProvider: async () => 'owner-token',
    onAuthenticationFailure: overrides.onAuthenticationFailure ?? vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
