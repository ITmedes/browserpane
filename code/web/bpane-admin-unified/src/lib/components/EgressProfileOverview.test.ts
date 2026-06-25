import { afterEach, describe, expect, it, vi } from 'vitest';

import type { EgressProfileResource } from '$lib/egress-profiles/egress-profile-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import EgressProfileOverview from './EgressProfileOverview.svelte';

afterEach(cleanupRenderedComponents);

describe('EgressProfileOverview', () => {
  it('renders loading, error, and empty states', async () => {
    let target = renderComponent(EgressProfileOverview, { state: { status: 'loading' } });
    expect(byTestId(target, 'egress-profiles-loading').textContent).toContain('Loading egress profiles');

    await cleanupRenderedComponents();
    target = renderComponent(EgressProfileOverview, {
      state: { status: 'error', message: 'No active admin access token is available.' },
    });
    expect(byTestId(target, 'egress-profiles-error').textContent).toContain('Egress profile catalog unavailable');

    await cleanupRenderedComponents();
    target = renderComponent(EgressProfileOverview, { state: { status: 'ready', profiles: [] } });
    expect(byTestId(target, 'egress-profiles-empty').textContent).toContain('Egress profile catalog is empty');
  });

  it('renders metrics, the catalog, and refresh action', () => {
    const onRefresh = vi.fn();
    const target = renderComponent(EgressProfileOverview, {
      state: {
        status: 'ready',
        profiles: [profile()],
      },
      onRefresh,
    });

    expect(byTestId(target, 'egress-profiles-metric-total').textContent).toContain('1');
    expect(byTestId(target, 'egress-profiles-metric-ready').textContent).toContain('1');
    expect(byTestId(target, 'egress-profiles-metric-tls').textContent).toContain('1');
    expect(byTestId(target, 'egress-profiles-list').textContent).toContain('EU support egress');
    expect(byTestId(target, 'egress-profiles-list').textContent).toContain('TLS interceptor');

    byTestId(target, 'egress-profiles-refresh-button').click();
    expect(onRefresh).toHaveBeenCalledOnce();
  });
});

function profile(): EgressProfileResource {
  return {
    id: 'profile-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'EU Support', state: 'active' },
    name: 'EU support egress',
    description: 'Support outbound path',
    labels: { region: 'eu' },
    proxy: { url: 'http://proxy.example:3128', credential_binding_id: 'credential-1' },
    bypass_rules: ['localhost'],
    custom_ca: { certificate_ref: 'file:///workspace/dev/egress-ca.pem', display_name: 'Local CA' },
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
