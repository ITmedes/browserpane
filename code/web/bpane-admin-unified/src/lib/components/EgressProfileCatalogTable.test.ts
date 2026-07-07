import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';

import type { EgressProfileResource } from '$lib/egress-profiles/egress-profile-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import EgressProfileCatalogTable from './EgressProfileCatalogTable.svelte';

afterEach(cleanupRenderedComponents);

describe('EgressProfileCatalogTable', () => {
  it('renders profile rows and filters by lens and search text', async () => {
    const target = renderComponent(EgressProfileCatalogTable, {
      profiles: [
        profile({ id: 'tls-profile', name: 'TLS profile', tls: true, projectName: 'Support' }),
        profile({ id: 'direct-profile', name: 'Direct profile', state: 'disabled', health: 'unknown' }),
      ],
    });

    expect(byTestId(target, 'egress-profiles-list-count').textContent).toContain('2 of 2');
    expect(target.querySelectorAll('[data-testid="egress-profiles-list-row"]')).toHaveLength(2);
    expect(byTestId(target, 'egress-profiles-list').textContent).toContain('TLS interceptor');
    expect(byTestId(target, 'egress-profiles-list').textContent).toContain('Direct');
    expect((target.querySelector('[data-testid="egress-profiles-detail-link"]') as HTMLAnchorElement).href).toBe(
      'http://localhost:3000/admin-new/egress/tls-profile',
    );

    byTestId(target, 'egress-profiles-lens-tls').click();
    await tick();

    expect(byTestId(target, 'egress-profiles-list-count').textContent).toContain('1 of 2');
    expect(target.querySelectorAll('[data-testid="egress-profiles-list-row"]')).toHaveLength(1);
    expect(byTestId(target, 'egress-profiles-list').textContent).toContain('TLS profile');
    expect(byTestId(target, 'egress-profiles-list').textContent).not.toContain('Direct profile');

    byTestId(target, 'egress-profiles-lens-all').click();
    const search = byTestId(target, 'egress-profiles-search') as HTMLInputElement;
    search.value = 'missing';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();

    expect(byTestId(target, 'egress-profiles-filter-empty').textContent).toContain('No egress profiles match');
  });
});

function profile(options: {
  readonly id: string;
  readonly name: string;
  readonly proxy?: boolean;
  readonly tls?: boolean;
  readonly state?: EgressProfileResource['state'];
  readonly health?: EgressProfileResource['diagnostics']['health'];
  readonly projectName?: string;
}): EgressProfileResource {
  const proxyConfigured = options.proxy === true || options.tls === true;
  return {
    id: options.id,
    project_id: options.projectName ? 'project-1' : null,
    project: options.projectName ? { id: 'project-1', name: options.projectName, state: 'active' } : null,
    name: options.name,
    description: `${options.name} outbound path`,
    labels: {},
    proxy: proxyConfigured ? { url: 'http://proxy.example:3128', credential_binding_id: null } : null,
    bypass_rules: proxyConfigured ? ['localhost'] : [],
    custom_ca: options.tls ? { certificate_ref: 'file:///workspace/dev/egress-ca.pem', display_name: null } : null,
    traffic_observation: {
      mode: options.tls ? 'tls_intercept' : 'metadata_only',
      sensitive_log_sink_ref: options.tls ? 'siem://browserpane/support' : null,
      sensitive_log_sink_display_name: null,
    },
    state: options.state ?? 'ready',
    effective: {
      proxy_configured: proxyConfigured,
      proxy_auth_configured: false,
      bypass_rule_count: proxyConfigured ? 1 : 0,
      custom_ca_configured: options.tls === true,
      observation_mode: options.tls ? 'tls_intercept' : 'metadata_only',
      tls_interception_enabled: options.tls === true,
      sensitive_log_sink_configured: options.tls === true,
    },
    diagnostics: {
      profile_id: options.id,
      profile_name: options.name,
      profile_state: options.state ?? 'ready',
      health: options.health ?? 'ready',
      observation_mode: options.tls ? 'tls_intercept' : 'metadata_only',
      proof_level: 'configuration',
      runtime_binding: null,
      runtime_assignment: null,
      proxy_configured: proxyConfigured,
      proxy_auth_configured: false,
      bypass_rule_count: proxyConfigured ? 1 : 0,
      custom_ca_configured: options.tls === true,
      tls_interception_enabled: options.tls === true,
      sensitive_log_sink_configured: options.tls === true,
      proof: {
        profile_resolved: true,
        profile_ready: options.state !== 'disabled',
        profile_reachability_collected: false,
        profile_reachability_healthy: false,
        profile_reachability_observed_at: null,
        profile_reachability_failure: null,
        proxy_launch_config_expected: proxyConfigured,
        bypass_rules_expected: proxyConfigured ? 1 : 0,
        custom_ca_launch_config_expected: options.tls === true,
        tls_interception_expected: options.tls === true,
        sensitive_log_sink_declared: options.tls === true,
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
