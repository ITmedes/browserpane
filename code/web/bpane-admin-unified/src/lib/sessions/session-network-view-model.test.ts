import { describe, expect, it } from 'vitest';

import { toEgressDiagnosticsResource } from '$lib/egress-profiles/egress-profile-client';
import { egressDiagnosticsPayload } from '$lib/test-utils/egress-fixtures';
import { sessionResource } from '$lib/test-utils/session-fixtures';
import { buildSessionNetworkModel } from './session-network-view-model';

describe('buildSessionNetworkModel', () => {
  it('represents ready proxy evidence and allows an active probe', () => {
    const diagnostics = toEgressDiagnosticsResource(egressDiagnosticsPayload());

    const model = buildSessionNetworkModel(sessionResource({ state: 'ready', runtimeState: 'running' }), diagnostics);

    expect(model).toMatchObject({
      mode: 'proxy',
      modeLabel: 'Forward proxy',
      health: 'ready',
      healthTone: 'success',
      proofLevel: 'runtime launch metadata',
      profileHref: '/admin-new/egress/egress-1',
      canProbe: true,
    });
    expect(model.proofFacts.find((fact) => fact.testId === 'session-network-runtime-binding')?.value)
      .toBe('docker:browser-1');
  });

  it('represents TLS interception without exposing secret configuration', () => {
    const diagnostics = toEgressDiagnosticsResource(egressDiagnosticsPayload({
      observation_mode: 'tls_intercept',
      custom_ca_configured: true,
      tls_interception_enabled: true,
      sensitive_log_sink_configured: true,
    }));

    const model = buildSessionNetworkModel(sessionResource(), diagnostics);

    expect(model.mode).toBe('tls_intercept');
    expect(model.effectiveFacts).toContainEqual(expect.objectContaining({
      testId: 'session-network-custom-ca',
      value: 'Enabled',
    }));
    expect(JSON.stringify(model)).not.toContain('certificate_ref');
  });

  it('represents direct egress without a profile link', () => {
    const diagnostics = toEgressDiagnosticsResource(egressDiagnosticsPayload({
      profile_id: null,
      profile_name: null,
      profile_state: null,
      proxy_configured: false,
      runtime_binding: 'static_single',
    }));

    const baseSession = sessionResource({ projectName: null });
    const model = buildSessionNetworkModel({
      ...baseSession,
      network_identity: { ...baseSession.network_identity, egress_profile_id: null },
      effective_egress: {
        profile_id: null,
        profile_name: null,
        profile_state: null,
        proxy_configured: false,
        proxy_auth_configured: false,
        bypass_rule_count: 0,
        custom_ca_configured: false,
        observation_mode: 'metadata_only',
        tls_interception_enabled: false,
        sensitive_log_sink_configured: false,
      },
    }, diagnostics);

    expect(model).toMatchObject({ mode: 'direct', modeLabel: 'Direct egress', profileHref: null });
  });

  it('blocks probes for stopped and not-ready runtimes while retaining warnings', () => {
    const diagnostics = toEgressDiagnosticsResource(egressDiagnosticsPayload({
      health: 'attention',
      runtime_assignment: 'starting',
      warnings: ['Runtime assignment is still starting.'],
      proof: {
        ...egressDiagnosticsPayload().proof as Record<string, unknown>,
        last_failure_reason: 'Previous active probe timed out.',
      },
    }));

    const model = buildSessionNetworkModel(
      sessionResource({ state: 'stopped', runtimeState: 'stopped', totalClients: 0 }),
      diagnostics,
    );

    expect(model.canProbe).toBe(false);
    expect(model.probeBlockedReason).toContain('Start and connect');
    expect(model.warnings).toEqual([
      'Runtime assignment is still starting.',
      'Previous active probe timed out.',
    ]);
  });
});
