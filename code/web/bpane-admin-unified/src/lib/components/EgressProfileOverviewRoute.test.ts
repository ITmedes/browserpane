import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import EgressProfileOverviewRoute from './EgressProfileOverviewRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/egress');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('EgressProfileOverviewRoute', () => {
  it('loads egress profiles with the authenticated shell token provider', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse(egressProfileListPayload(), 200));
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(EgressProfileOverviewRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'egress-profiles-list').textContent).toContain('EU support egress');
    });
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(new URL('http://localhost:3000/api/v1/egress-profiles'));
    expect(fetchImpl.mock.calls[0]?.[1]?.method).toBe('GET');
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(EgressProfileOverviewRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'egress-profiles-error').textContent).toContain('Egress profile catalog unavailable');
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });
});

function authContext(
  overrides: Partial<Pick<UnifiedAdminContext, 'accessTokenProvider' | 'onAuthenticationFailure'>> = {},
): UnifiedAdminContext {
  return {
    auth: {
      configured: true,
      authenticated: true,
      username: 'demo',
      accessToken: 'token',
      claims: null,
    },
    authConfig: null,
    accessTokenProvider: overrides.accessTokenProvider ?? (async () => 'token'),
    onAuthenticationFailure: overrides.onAuthenticationFailure ?? vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function egressProfileListPayload() {
  return {
    profiles: [egressProfilePayload()],
  };
}

function egressProfilePayload() {
  return {
    id: 'profile-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'EU Support', state: 'active' },
    name: 'EU support egress',
    description: 'Support outbound path',
    labels: { region: 'eu' },
    proxy: {
      url: 'http://proxy.example:3128',
      credential_binding_id: 'credential-1',
    },
    bypass_rules: ['localhost'],
    custom_ca: {
      certificate_ref: 'file:///workspace/dev/egress-ca.pem',
      display_name: 'Local CA',
    },
    traffic_observation: {
      mode: 'tls_intercept',
      sensitive_log_sink_ref: 'siem://browserpane/eu-support',
      sensitive_log_sink_display_name: 'EU support SIEM',
    },
    state: 'ready',
    effective: {
      proxy_configured: true,
      proxy_auth_configured: true,
      bypass_rule_count: 1,
      custom_ca_configured: true,
      observation_mode: 'tls_intercept',
      tls_interception_enabled: true,
      sensitive_log_sink_configured: true,
    },
    diagnostics: {
      profile_id: 'profile-1',
      profile_name: 'EU support egress',
      profile_state: 'ready',
      health: 'ready',
      observation_mode: 'tls_intercept',
      proof_level: 'configuration',
      runtime_binding: null,
      runtime_assignment: null,
      proxy_configured: true,
      proxy_auth_configured: true,
      bypass_rule_count: 1,
      custom_ca_configured: true,
      tls_interception_enabled: true,
      sensitive_log_sink_configured: true,
      proof: {
        profile_resolved: true,
        profile_ready: true,
        profile_reachability_collected: false,
        profile_reachability_healthy: false,
        profile_reachability_observed_at: null,
        profile_reachability_failure: null,
        proxy_launch_config_expected: true,
        bypass_rules_expected: 1,
        custom_ca_launch_config_expected: true,
        tls_interception_expected: true,
        sensitive_log_sink_declared: true,
        runtime_launch_observed: false,
        active_probe_collected: false,
        observed_public_ip: null,
        observed_tls_issuer: null,
        last_failure_reason: null,
      },
      warnings: [],
      observed_at: '2026-06-12T10:00:00.000Z',
    },
    created_at: '2026-06-12T09:00:00.000Z',
    updated_at: '2026-06-12T10:00:00.000Z',
  };
}
