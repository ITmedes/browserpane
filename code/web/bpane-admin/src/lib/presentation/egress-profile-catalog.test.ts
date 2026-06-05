import { describe, expect, it } from 'vitest';
import type { EgressProfileResource } from '../api/control-types';
import {
  buildEgressProfileCommand,
  commandFromEgressProfile,
  egressProfileRows,
  formFromEgressProfile,
} from './egress-profile-catalog';

const PROFILE: EgressProfileResource = {
  id: 'profile-1',
  project_id: '019e4faf-77ab-728d-9cd2-54e21d377b46',
  project: {
    id: '019e4faf-77ab-728d-9cd2-54e21d377b46',
    name: 'EU Support',
    state: 'active',
  },
  name: 'EU support egress',
  description: 'Support outbound path',
  labels: { region: 'eu' },
  proxy: {
    url: 'http://proxy.example:3128',
    credential_binding_id: '019e4faf-5171-75c3-9927-28fa1c982aa1',
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
    observed_at: '2026-05-22T08:10:00Z',
  },
  created_at: '2026-05-22T08:00:00Z',
  updated_at: '2026-05-22T08:10:00Z',
};

describe('egress profile catalog helpers', () => {
  it('builds rows with searchable operational labels', () => {
    const rows = egressProfileRows([PROFILE], 'tls');

    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      id: 'profile-1',
      kind: 'tls',
      health: 'ready',
      proofLevel: 'configuration',
      projectLabel: 'EU Support',
      badges: ['proxy', 'proxy auth', 'TLS inspect', 'custom CA', 'log sink', 'project scoped', 'config proof'],
    });
  });

  it('creates a valid tls_intercept command from form input', () => {
    const result = buildEgressProfileCommand({
      projectId: '019e4faf-77ab-728d-9cd2-54e21d377b46',
      name: 'Local TLS',
      description: 'Local interception profile',
      labels: 'browserpane.local=true\nregion=local',
      proxyUrl: 'http://bpane-egress-tls-observer:3129',
      proxyCredentialBindingId: '019e4faf-5171-75c3-9927-28fa1c982aa1',
      bypassRules: 'localhost,*.local',
      customCaRef: 'file:///workspace/dev/egress-ca.pem',
      customCaName: 'Local CA',
      observationMode: 'tls_intercept',
      sensitiveLogSinkRef: 'siem://browserpane/local-egress',
      sensitiveLogSinkName: 'Local SIEM',
      state: 'ready',
    });

    expect(result).toMatchObject({
      ok: true,
      command: {
        name: 'Local TLS',
        project_id: '019e4faf-77ab-728d-9cd2-54e21d377b46',
        labels: { 'browserpane.local': 'true', region: 'local' },
        proxy: {
          url: 'http://bpane-egress-tls-observer:3129',
          credential_binding_id: '019e4faf-5171-75c3-9927-28fa1c982aa1',
        },
        bypass_rules: ['localhost', '*.local'],
        custom_ca: {
          certificate_ref: 'file:///workspace/dev/egress-ca.pem',
          display_name: 'Local CA',
        },
        traffic_observation: {
          mode: 'tls_intercept',
          sensitive_log_sink_ref: 'siem://browserpane/local-egress',
          sensitive_log_sink_display_name: 'Local SIEM',
        },
      },
    });
  });

  it('rejects incomplete tls_intercept form input before hitting the API', () => {
    const result = buildEgressProfileCommand({
      projectId: '',
      name: 'Bad TLS',
      description: '',
      labels: '',
      proxyUrl: 'http://proxy.example:3128',
      proxyCredentialBindingId: '',
      bypassRules: '',
      customCaRef: '',
      customCaName: '',
      observationMode: 'tls_intercept',
      sensitiveLogSinkRef: '',
      sensitiveLogSinkName: '',
      state: 'ready',
    });

    expect(result).toEqual({
      ok: false,
      error: 'TLS interception requires a custom CA reference.',
    });
  });

  it('turns an existing profile into clone and disable payloads', () => {
    expect(formFromEgressProfile(PROFILE, { clone: true }).name).toBe('EU support egress-copy');
    expect(formFromEgressProfile(PROFILE, { clone: true }).projectId).toBe(PROFILE.project_id);
    expect(commandFromEgressProfile(PROFILE, 'disabled')).toMatchObject({
      name: 'EU support egress',
      project_id: PROFILE.project_id,
      state: 'disabled',
      traffic_observation: {
        mode: 'tls_intercept',
        sensitive_log_sink_ref: 'siem://browserpane/eu-support',
      },
    });
  });
});
