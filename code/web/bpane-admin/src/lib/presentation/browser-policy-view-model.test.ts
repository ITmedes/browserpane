import { describe, expect, it } from 'vitest';
import type { SessionResource } from '../api/control-types';
import { BrowserPolicyViewModelBuilder } from './browser-policy-view-model';

describe('BrowserPolicyViewModelBuilder', () => {
  it('reports docker-backed sessions as managed deny_all policy', () => {
    const viewModel = BrowserPolicyViewModelBuilder.build(sessionResource({
      binding: 'docker_runtime_pool',
      compatibilityMode: 'session_runtime_pool',
      cdpEndpoint: 'http://runtime:9223',
    }));

    expect(viewModel.mode).toBe('deny_all');
    expect(viewModel.canCopyProbeCommand).toBe(true);
    expect(viewModel.probeCommand).toContain('cdp-local-file-policy-probe.mjs');
    expect(viewModel.signals.map((signal) => signal.value)).toEqual([
      'blocked',
      'blocked',
      'blocked',
    ]);
  });

  it('does not overstate policy guarantees for legacy runtimes', () => {
    const viewModel = BrowserPolicyViewModelBuilder.build(sessionResource({
      binding: 'static_single',
      compatibilityMode: 'legacy_single',
      cdpEndpoint: null,
    }));

    expect(viewModel.mode).toBe('unknown');
    expect(viewModel.canCopyProbeCommand).toBe(false);
    expect(viewModel.signals.every((signal) => signal.tone === 'neutral')).toBe(true);
  });

  it('prompts for a selected session before policy inspection', () => {
    const viewModel = BrowserPolicyViewModelBuilder.build(null);

    expect(viewModel.title).toBe('No session selected');
    expect(viewModel.canRefresh).toBe(false);
    expect(viewModel.mode).toBe('unknown');
  });
});

function sessionResource(input: {
  readonly binding: string;
  readonly compatibilityMode: string;
  readonly cdpEndpoint: string | null;
}): SessionResource {
  return {
    id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
    state: 'active',
    owner_mode: 'shared',
    automation_delegate: null,
    connect: {
      gateway_url: 'https://localhost:4433',
      transport_path: '/session',
      auth_type: 'session_connect_ticket',
      compatibility_mode: input.compatibilityMode,
    },
    runtime: {
      binding: input.binding,
      compatibility_mode: input.compatibilityMode,
      cdp_endpoint: input.cdpEndpoint,
    },
    status: {
      runtime_state: 'running',
      presence_state: 'connected',
      connection_counts: {
        interactive_clients: 0,
        owner_clients: 0,
        viewer_clients: 0,
        recorder_clients: 0,
        automation_clients: 0,
        total_clients: 0,
      },
      stop_eligibility: { allowed: true, blockers: [] },
    },
    created_at: '2026-05-04T19:00:00Z',
    updated_at: '2026-05-04T19:01:00Z',
    stopped_at: null,
  };
}
