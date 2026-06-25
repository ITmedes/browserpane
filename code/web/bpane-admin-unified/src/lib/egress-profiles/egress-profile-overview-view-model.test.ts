import { describe, expect, it } from 'vitest';

import {
  buildEgressProfileOverviewModel,
  egressProfileMatchesSearch,
} from './egress-profile-overview-view-model';
import type { EgressProfileResource } from './egress-profile-types';

describe('egress profile overview view model', () => {
  it('builds metrics and rows from proxy, tls, and direct profiles', () => {
    const model = buildEgressProfileOverviewModel([
      profile({ id: 'tls', name: 'TLS profile', tls: true, health: 'ready' }),
      profile({ id: 'proxy', name: 'Proxy profile', proxy: true, health: 'attention' }),
      profile({ id: 'direct', name: 'Direct profile', state: 'disabled', health: 'unknown' }),
    ]);

    expect(model.metrics).toEqual([
      { label: 'Profiles', value: '3', testId: 'egress-profiles-metric-total' },
      { label: 'Ready', value: '2', testId: 'egress-profiles-metric-ready' },
      { label: 'TLS intercept', value: '1', testId: 'egress-profiles-metric-tls' },
      { label: 'Needs attention', value: '2', testId: 'egress-profiles-metric-attention' },
    ]);
    expect(model.rows.map((row) => ({
      id: row.id,
      kind: row.kind,
      kindLabel: row.kindLabel,
      healthTone: row.healthTone,
    }))).toEqual([
      { id: 'tls', kind: 'tls', kindLabel: 'TLS interceptor', healthTone: 'success' },
      { id: 'proxy', kind: 'proxy', kindLabel: 'Egress proxy', healthTone: 'warning' },
      { id: 'direct', kind: 'direct', kindLabel: 'Direct', healthTone: 'warning' },
    ]);
  });

  it('matches search against operational profile labels', () => {
    const row = buildEgressProfileOverviewModel([
      profile({ id: 'profile-1', name: 'EU Support', tls: true, projectName: 'Customer Support' }),
    ]).rows[0];

    expect(row).toBeDefined();
    if (!row) {
      return;
    }

    expect(egressProfileMatchesSearch(row, 'customer')).toBe(true);
    expect(egressProfileMatchesSearch(row, 'tls inspect')).toBe(true);
    expect(egressProfileMatchesSearch(row, 'configuration')).toBe(true);
    expect(egressProfileMatchesSearch(row, 'missing')).toBe(false);
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
    description: 'Outbound profile',
    labels: {},
    proxy: proxyConfigured ? { url: 'http://proxy.example:3128' } : null,
    bypass_rules: proxyConfigured ? ['localhost'] : [],
    custom_ca: options.tls ? { certificate_ref: 'file:///workspace/dev/egress-ca.pem' } : null,
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
