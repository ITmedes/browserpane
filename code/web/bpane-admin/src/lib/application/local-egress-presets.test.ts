import { describe, expect, it, vi } from 'vitest';
import type { ControlClient } from '../api/control-client';
import type { CreateEgressProfileCommand, EgressProfileResource } from '../api/control-types';
import {
  ensureLocalEgressPresets,
  localEgressPresetCommands,
  shouldEnsureLocalEgressPresets,
} from './local-egress-presets';

describe('local egress presets', () => {
  it('is enabled only for local admin hosts', () => {
    expect(shouldEnsureLocalEgressPresets({ hostname: 'localhost' })).toBe(true);
    expect(shouldEnsureLocalEgressPresets({ hostname: '127.0.0.1' })).toBe(true);
    expect(shouldEnsureLocalEgressPresets({ hostname: 'admin.browserpane.example' })).toBe(false);
    expect(shouldEnsureLocalEgressPresets(null)).toBe(false);
  });

  it('creates proxy and TLS-interceptor profiles for a local admin session', async () => {
    const created: EgressProfileResource[] = [];
    const client = {
      createEgressProfile: vi.fn(async (command: CreateEgressProfileCommand) => {
        const profile = profileFromCommand(command, `profile-${created.length + 1}`);
        created.push(profile);
        return profile;
      }),
      listEgressProfiles: vi.fn(async () => ({ profiles: created })),
    } as unknown as ControlClient;

    const result = await ensureLocalEgressPresets(client, [], { hostname: 'localhost' });

    expect(result.created).toBe(2);
    expect(result.error).toBeUndefined();
    expect(result.profiles).toHaveLength(2);
    expect(client.createEgressProfile).toHaveBeenCalledTimes(2);
    expect(client.createEgressProfile).toHaveBeenNthCalledWith(1, expect.objectContaining({
      name: 'Local: Egress as Proxy',
      proxy: { url: 'http://bpane-egress-observer:3128' },
      traffic_observation: { mode: 'metadata_only' },
    }));
    expect(client.createEgressProfile).toHaveBeenNthCalledWith(2, expect.objectContaining({
      name: 'Local: Egress as TLS Interceptor',
      proxy: { url: 'http://bpane-egress-tls-observer:3129' },
      custom_ca: {
        certificate_ref: 'file:///workspace/dev/egress-ca.pem',
        display_name: 'BrowserPane Local Egress Test CA',
      },
      traffic_observation: expect.objectContaining({ mode: 'tls_intercept' }),
    }));
  });

  it('does not duplicate existing local presets', async () => {
    const commands = localEgressPresetCommands();
    const profiles = commands.map((command, index) => profileFromCommand(command, `profile-${index + 1}`));
    const client = {
      createEgressProfile: vi.fn(),
      listEgressProfiles: vi.fn(),
    } as unknown as ControlClient;

    const result = await ensureLocalEgressPresets(client, profiles, { hostname: 'localhost' });

    expect(result.created).toBe(0);
    expect(result.profiles).toEqual(profiles);
    expect(client.createEgressProfile).not.toHaveBeenCalled();
  });

  it('leaves production profile lists untouched', async () => {
    const client = {
      createEgressProfile: vi.fn(),
      listEgressProfiles: vi.fn(),
    } as unknown as ControlClient;

    const result = await ensureLocalEgressPresets(client, [], { hostname: 'admin.browserpane.example' });

    expect(result).toEqual({ profiles: [], created: 0, enabled: false });
    expect(client.createEgressProfile).not.toHaveBeenCalled();
  });
});

function profileFromCommand(command: CreateEgressProfileCommand, id: string): EgressProfileResource {
  const observationMode = command.traffic_observation?.mode ?? 'metadata_only';
  return {
    id,
    name: command.name,
    description: command.description ?? null,
    labels: command.labels ?? {},
    proxy: command.proxy ?? null,
    bypass_rules: command.bypass_rules ?? [],
    custom_ca: command.custom_ca ?? null,
    traffic_observation: command.traffic_observation ?? { mode: 'metadata_only' },
    state: command.state ?? 'ready',
    effective: {
      proxy_configured: Boolean(command.proxy),
      bypass_rule_count: command.bypass_rules?.length ?? 0,
      custom_ca_configured: Boolean(command.custom_ca),
      observation_mode: observationMode,
      tls_interception_enabled: observationMode === 'tls_intercept',
      sensitive_log_sink_configured: Boolean(command.traffic_observation?.sensitive_log_sink_ref),
    },
    diagnostics: {
      profile_id: id,
      profile_name: command.name,
      profile_state: command.state ?? 'ready',
      health: 'ready',
      observation_mode: observationMode,
      proof_level: 'configuration',
      runtime_binding: null,
      runtime_assignment: null,
      proxy_configured: Boolean(command.proxy),
      bypass_rule_count: command.bypass_rules?.length ?? 0,
      custom_ca_configured: Boolean(command.custom_ca),
      tls_interception_enabled: observationMode === 'tls_intercept',
      sensitive_log_sink_configured: Boolean(command.traffic_observation?.sensitive_log_sink_ref),
      proof: {
        profile_resolved: true,
        profile_ready: (command.state ?? 'ready') === 'ready',
        proxy_launch_config_expected: Boolean(command.proxy),
        bypass_rules_expected: command.bypass_rules?.length ?? 0,
        custom_ca_launch_config_expected: Boolean(command.custom_ca) && observationMode === 'tls_intercept',
        tls_interception_expected: observationMode === 'tls_intercept',
        sensitive_log_sink_declared: Boolean(command.traffic_observation?.sensitive_log_sink_ref),
        runtime_launch_observed: false,
        active_probe_collected: false,
        observed_public_ip: null,
        observed_tls_issuer: null,
        last_failure_reason: null,
      },
      warnings: [],
      observed_at: '2026-05-22T10:00:00Z',
    },
    created_at: '2026-05-22T10:00:00Z',
    updated_at: '2026-05-22T10:00:00Z',
  };
}
