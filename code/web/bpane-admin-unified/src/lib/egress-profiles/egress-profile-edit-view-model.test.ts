import { describe, expect, it } from 'vitest';

import {
  createEgressProfileEditDraft,
  createNewEgressProfileEditDraft,
  hasEgressProfileEditChanges,
  hasNewEgressProfileEditChanges,
  mergeProjectsWithSelected,
  setEgressObservationMode,
  validateEgressProfileEdit,
} from './egress-profile-edit-view-model';
import type { EgressProfileResource } from './egress-profile-types';

describe('egress profile edit view model', () => {
  it('builds a direct metadata-only create request', () => {
    const draft = {
      ...createNewEgressProfileEditDraft(),
      name: 'Direct Support',
      description: '  Owner scoped profile  ',
      labelsText: 'team=support\nregion=eu',
      bypassRulesText: 'localhost\nlocalhost\n127.0.0.1',
    };

    const validation = validateEgressProfileEdit(null, draft);

    expect(validation.valid).toBe(true);
    expect(validation.request).toEqual({
      project_id: null,
      name: 'Direct Support',
      description: 'Owner scoped profile',
      labels: { team: 'support', region: 'eu' },
      proxy: null,
      bypass_rules: ['127.0.0.1', 'localhost'],
      custom_ca: null,
      traffic_observation: {
        mode: 'metadata_only',
        sensitive_log_sink_ref: null,
        sensitive_log_sink_display_name: null,
      },
      state: 'ready',
    });
    expect(hasNewEgressProfileEditChanges(draft)).toBe(true);
  });

  it('requires proxy, custom CA, and sensitive sink details for TLS intercept', () => {
    const draft = setEgressObservationMode(createNewEgressProfileEditDraft(), 'tls_intercept');

    const validation = validateEgressProfileEdit(null, draft);

    expect(validation.valid).toBe(false);
    expect(validation.fieldErrors.proxyUrl).toContain('Proxy URL is required.');
    expect(validation.fieldErrors.customCaCertificateRef).toContain('Custom CA certificate reference is required.');
    expect(validation.fieldErrors.sensitiveLogSinkRef).toContain(
      'TLS intercept requires an approved sensitive log sink reference.',
    );
  });

  it('builds an update request for project-scoped TLS interception', () => {
    const draft = {
      ...createEgressProfileEditDraft(profile()),
      name: 'TLS support profile',
      projectBinding: 'project' as const,
      projectId: '22222222-2222-4222-8222-222222222222',
      proxyEnabled: true,
      proxyUrl: 'http://proxy.example:3128',
      proxyCredentialBindingId: '33333333-3333-4333-8333-333333333333',
      customCaEnabled: true,
      customCaCertificateRef: 'file:///workspace/dev/egress-ca.pem',
      customCaDisplayName: 'Local CA',
      observationMode: 'tls_intercept' as const,
      sensitiveLogSinkRef: 'siem://browserpane/support',
      sensitiveLogSinkDisplayName: 'Support SIEM',
    };

    const validation = validateEgressProfileEdit(profile(), draft);

    expect(validation.valid).toBe(true);
    expect(validation.request).toMatchObject({
      project_id: '22222222-2222-4222-8222-222222222222',
      proxy: {
        url: 'http://proxy.example:3128',
        credential_binding_id: '33333333-3333-4333-8333-333333333333',
      },
      custom_ca: {
        certificate_ref: 'file:///workspace/dev/egress-ca.pem',
        display_name: 'Local CA',
      },
      traffic_observation: {
        mode: 'tls_intercept',
        sensitive_log_sink_ref: 'siem://browserpane/support',
        sensitive_log_sink_display_name: 'Support SIEM',
      },
    });
    expect(hasEgressProfileEditChanges(profile(), draft)).toBe(true);
  });

  it('validates unsafe proxy and sink references', () => {
    const draft = {
      ...createNewEgressProfileEditDraft(),
      name: 'Bad proxy',
      proxyEnabled: true,
      proxyUrl: 'http://user:pass@proxy.example:3128',
      proxyCredentialBindingId: 'not-a-uuid',
      sensitiveLogSinkRef: 'siem://user:pass@example',
    };

    const validation = validateEgressProfileEdit(null, draft);

    expect(validation.fieldErrors.proxyUrl).toContain('Proxy URL must not contain inline credentials.');
    expect(validation.fieldErrors.proxyCredentialBindingId).toContain('Proxy credential binding id must be a UUID.');
    expect(validation.fieldErrors.sensitiveLogSinkRef).toContain(
      'Sensitive log sink reference must not contain inline credentials.',
    );
  });

  it('keeps a missing selected project visible in project options', () => {
    const projects = mergeProjectsWithSelected([], '22222222-2222-4222-8222-222222222222');

    expect(projects).toEqual([{
      id: '22222222-2222-4222-8222-222222222222',
      name: 'Missing project 22222222...',
      state: 'archived',
    }]);
  });
});

function profile(): EgressProfileResource {
  return {
    id: 'profile-1',
    project_id: null,
    project: null,
    name: 'Direct profile',
    description: null,
    labels: {},
    proxy: null,
    bypass_rules: [],
    custom_ca: null,
    traffic_observation: {
      mode: 'metadata_only',
      sensitive_log_sink_ref: null,
      sensitive_log_sink_display_name: null,
    },
    state: 'ready',
    effective: {
      proxy_configured: false,
      proxy_auth_configured: false,
      bypass_rule_count: 0,
      custom_ca_configured: false,
      observation_mode: 'metadata_only',
      tls_interception_enabled: false,
      sensitive_log_sink_configured: false,
    },
    diagnostics: {
      profile_id: 'profile-1',
      profile_name: 'Direct profile',
      profile_state: 'ready',
      health: 'ready',
      observation_mode: 'metadata_only',
      proof_level: 'configuration',
      runtime_binding: null,
      runtime_assignment: null,
      proxy_configured: false,
      proxy_auth_configured: false,
      bypass_rule_count: 0,
      custom_ca_configured: false,
      tls_interception_enabled: false,
      sensitive_log_sink_configured: false,
      proof: {
        profile_resolved: true,
        profile_ready: true,
        profile_reachability_collected: false,
        profile_reachability_healthy: false,
        profile_reachability_observed_at: null,
        profile_reachability_failure: null,
        proxy_launch_config_expected: false,
        bypass_rules_expected: 0,
        custom_ca_launch_config_expected: false,
        tls_interception_expected: false,
        sensitive_log_sink_declared: false,
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
