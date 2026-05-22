import type { ControlClient } from '../api/control-client';
import {
  LOCAL_EGRESS_PRESET_LABEL_KEY,
  LOCAL_EGRESS_PROXY_NAME,
  LOCAL_EGRESS_PROXY_PRESET,
  LOCAL_EGRESS_TLS_NAME,
  LOCAL_EGRESS_TLS_PRESET,
} from '../api/local-egress-preset-types';
import type { CreateEgressProfileCommand, EgressProfileResource } from '../api/control-types';

export type LocalEgressPresetResult = {
  readonly profiles: readonly EgressProfileResource[];
  readonly created: number;
  readonly enabled: boolean;
  readonly error?: string | null;
};

export async function ensureLocalEgressPresets(
  controlClient: ControlClient,
  profiles: readonly EgressProfileResource[],
  location: Pick<Location, 'hostname'> | null = browserLocation(),
): Promise<LocalEgressPresetResult> {
  if (!shouldEnsureLocalEgressPresets(location)) {
    return { profiles, created: 0, enabled: false };
  }

  let nextProfiles: readonly EgressProfileResource[] = profiles;
  let created = 0;
  for (const command of localEgressPresetCommands()) {
    if (hasPreset(nextProfiles, command)) {
      continue;
    }
    try {
      const profile = await controlClient.createEgressProfile(command);
      nextProfiles = [profile, ...nextProfiles.filter((entry) => entry.id !== profile.id)];
      created += 1;
    } catch (error) {
      const refreshed = await refreshProfilesAfterPresetFailure(controlClient, nextProfiles);
      if (hasPreset(refreshed, command)) {
        nextProfiles = refreshed;
        continue;
      }
      return {
        profiles: refreshed,
        created,
        enabled: true,
        error: errorMessage(error),
      };
    }
  }

  return { profiles: nextProfiles, created, enabled: true };
}

export function shouldEnsureLocalEgressPresets(
  location: Pick<Location, 'hostname'> | null,
): boolean {
  if (!location) {
    return false;
  }
  return ['localhost', '127.0.0.1', '::1'].includes(location.hostname);
}

export function localEgressPresetCommands(): readonly CreateEgressProfileCommand[] {
  return [
    {
      name: LOCAL_EGRESS_PROXY_NAME,
      description: 'Local metadata-only forward proxy preset for BrowserPane egress testing.',
      labels: {
        [LOCAL_EGRESS_PRESET_LABEL_KEY]: LOCAL_EGRESS_PROXY_PRESET,
      },
      proxy: { url: 'http://bpane-egress-observer:3128' },
      bypass_rules: ['localhost', '127.0.0.1', '*.local'],
      traffic_observation: { mode: 'metadata_only' },
      state: 'ready',
    },
    {
      name: LOCAL_EGRESS_TLS_NAME,
      description: 'Local mitmproxy TLS-interception preset for BrowserPane egress testing.',
      labels: {
        [LOCAL_EGRESS_PRESET_LABEL_KEY]: LOCAL_EGRESS_TLS_PRESET,
      },
      proxy: { url: 'http://bpane-egress-tls-observer:3129' },
      bypass_rules: ['localhost', '127.0.0.1', '*.local'],
      custom_ca: {
        certificate_ref: 'file:///workspace/dev/egress-ca.pem',
        display_name: 'BrowserPane Local Egress Test CA',
      },
      traffic_observation: {
        mode: 'tls_intercept',
        sensitive_log_sink_ref: 'siem://browserpane/local-egress',
        sensitive_log_sink_display_name: 'Local Egress SIEM',
      },
      state: 'ready',
    },
  ];
}

function hasPreset(
  profiles: readonly EgressProfileResource[],
  command: CreateEgressProfileCommand,
): boolean {
  const preset = command.labels?.[LOCAL_EGRESS_PRESET_LABEL_KEY];
  return profiles.some((profile) => {
    if (profile.name === command.name) {
      return true;
    }
    return Boolean(preset && profile.labels[LOCAL_EGRESS_PRESET_LABEL_KEY] === preset);
  });
}

async function refreshProfilesAfterPresetFailure(
  controlClient: ControlClient,
  fallback: readonly EgressProfileResource[],
): Promise<readonly EgressProfileResource[]> {
  try {
    return (await controlClient.listEgressProfiles()).profiles;
  } catch {
    return fallback;
  }
}

function browserLocation(): Pick<Location, 'hostname'> | null {
  if (typeof window === 'undefined') {
    return null;
  }
  return window.location;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Local egress preset creation failed.';
}
