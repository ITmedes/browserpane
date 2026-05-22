import type {
  CreateEgressProfileCommand,
  EgressProfileResource,
  EgressProfileState,
  EgressTrafficObservationMode,
} from '../api/control-types';

export type EgressProfileFormInput = {
  readonly name: string;
  readonly description: string;
  readonly labels: string;
  readonly proxyUrl: string;
  readonly proxyCredentialBindingId: string;
  readonly bypassRules: string;
  readonly customCaRef: string;
  readonly customCaName: string;
  readonly observationMode: EgressTrafficObservationMode;
  readonly sensitiveLogSinkRef: string;
  readonly sensitiveLogSinkName: string;
  readonly state: EgressProfileState;
};

export type EgressProfileCommandResult =
  | { readonly ok: true; readonly command: CreateEgressProfileCommand }
  | { readonly ok: false; readonly error: string };

export type EgressProfileCatalogRow = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly state: EgressProfileState;
  readonly health: string;
  readonly proofLevel: string;
  readonly kind: 'proxy' | 'tls' | 'direct';
  readonly badges: readonly string[];
  readonly updatedAt: string;
};

export function egressProfileRows(
  profiles: readonly EgressProfileResource[],
  search: string,
): readonly EgressProfileCatalogRow[] {
  const needle = search.trim().toLowerCase();
  return profiles
    .map((profile) => ({
      id: profile.id,
      name: profile.name,
      description: profile.description ?? '',
      state: profile.state,
      health: profile.diagnostics.health,
      proofLevel: profile.diagnostics.proof_level,
      kind: profile.effective.tls_interception_enabled
        ? 'tls' as const
        : profile.effective.proxy_configured
          ? 'proxy' as const
          : 'direct' as const,
      badges: egressProfileBadges(profile),
      updatedAt: profile.updated_at,
    }))
    .filter((row) => {
      if (!needle) {
        return true;
      }
      return [
        row.id,
        row.name,
        row.description,
        row.state,
        row.health,
        row.proofLevel,
        row.kind,
        ...row.badges,
      ].some((value) => value.toLowerCase().includes(needle));
    });
}

export function emptyEgressProfileForm(): EgressProfileFormInput {
  return {
    name: '',
    description: '',
    labels: '',
    proxyUrl: '',
    proxyCredentialBindingId: '',
    bypassRules: '',
    customCaRef: '',
    customCaName: '',
    observationMode: 'metadata_only',
    sensitiveLogSinkRef: '',
    sensitiveLogSinkName: '',
    state: 'ready',
  };
}

export function formFromEgressProfile(
  profile: EgressProfileResource,
  options: { readonly clone?: boolean } = {},
): EgressProfileFormInput {
  return {
    name: options.clone ? `${profile.name}-copy` : profile.name,
    description: profile.description ?? '',
    labels: Object.entries(profile.labels)
      .map(([key, value]) => `${key}=${value}`)
      .join('\n'),
    proxyUrl: profile.proxy?.url ?? '',
    proxyCredentialBindingId: profile.proxy?.credential_binding_id ?? '',
    bypassRules: profile.bypass_rules.join('\n'),
    customCaRef: profile.custom_ca?.certificate_ref ?? '',
    customCaName: profile.custom_ca?.display_name ?? '',
    observationMode: profile.traffic_observation.mode,
    sensitiveLogSinkRef: profile.traffic_observation.sensitive_log_sink_ref ?? '',
    sensitiveLogSinkName: profile.traffic_observation.sensitive_log_sink_display_name ?? '',
    state: options.clone ? 'ready' : profile.state,
  };
}

export function buildEgressProfileCommand(input: EgressProfileFormInput): EgressProfileCommandResult {
  const name = input.name.trim();
  if (!name) {
    return { ok: false, error: 'Profile name is required.' };
  }

  const labels = parseLabels(input.labels);
  if (!labels.ok) {
    return labels;
  }

  const proxyUrl = input.proxyUrl.trim();
  const proxyCredentialBindingId = input.proxyCredentialBindingId.trim();
  const customCaRef = input.customCaRef.trim();
  const customCaName = input.customCaName.trim();
  const sensitiveLogSinkRef = input.sensitiveLogSinkRef.trim();
  const sensitiveLogSinkName = input.sensitiveLogSinkName.trim();
  if (customCaName && !customCaRef) {
    return { ok: false, error: 'Custom CA display name requires a certificate reference.' };
  }
  if (sensitiveLogSinkName && !sensitiveLogSinkRef) {
    return { ok: false, error: 'Log-sink display name requires a log-sink reference.' };
  }
  if (input.observationMode === 'tls_intercept') {
    if (!proxyUrl) {
      return { ok: false, error: 'TLS interception requires a proxy URL.' };
    }
    if (!customCaRef) {
      return { ok: false, error: 'TLS interception requires a custom CA reference.' };
    }
    if (!sensitiveLogSinkRef) {
      return { ok: false, error: 'TLS interception requires a sensitive log-sink reference.' };
    }
  }
  if (proxyCredentialBindingId && !proxyUrl) {
    return { ok: false, error: 'Proxy auth binding requires a proxy URL.' };
  }

  const description = input.description.trim();
  const proxy = proxyUrl
    ? {
        url: proxyUrl,
        ...(proxyCredentialBindingId ? { credential_binding_id: proxyCredentialBindingId } : {}),
      }
    : null;
  const command: CreateEgressProfileCommand = {
    name,
    ...(description ? { description } : {}),
    labels: labels.value,
    ...(proxy ? { proxy } : {}),
    bypass_rules: splitList(input.bypassRules),
    ...(customCaRef
      ? {
          custom_ca: {
            certificate_ref: customCaRef,
            ...(customCaName ? { display_name: customCaName } : {}),
          },
        }
      : {}),
    traffic_observation: {
      mode: input.observationMode,
      ...(sensitiveLogSinkRef ? { sensitive_log_sink_ref: sensitiveLogSinkRef } : {}),
      ...(sensitiveLogSinkName ? { sensitive_log_sink_display_name: sensitiveLogSinkName } : {}),
    },
    state: input.state,
  };
  return { ok: true, command };
}

export function commandFromEgressProfile(
  profile: EgressProfileResource,
  state: EgressProfileState = profile.state,
): CreateEgressProfileCommand {
  return {
    name: profile.name,
    ...(profile.description ? { description: profile.description } : {}),
    labels: profile.labels,
    ...(profile.proxy ? { proxy: profile.proxy } : {}),
    bypass_rules: profile.bypass_rules,
    ...(profile.custom_ca ? { custom_ca: profile.custom_ca } : {}),
    traffic_observation: profile.traffic_observation,
    state,
  };
}

export function egressProfileBadges(profile: EgressProfileResource): readonly string[] {
  return [
    profile.effective.proxy_configured ? 'proxy' : 'direct',
    profile.effective.proxy_auth_configured ? 'proxy auth' : null,
    profile.effective.tls_interception_enabled ? 'TLS inspect' : null,
    profile.effective.custom_ca_configured ? 'custom CA' : null,
    profile.effective.sensitive_log_sink_configured ? 'log sink' : null,
    profile.state === 'disabled' ? 'disabled' : null,
    profile.diagnostics.health !== 'ready' ? `health ${profile.diagnostics.health}` : null,
    profile.diagnostics.proof_level === 'active_probe'
      ? 'active proof'
      : profile.diagnostics.proof_level === 'runtime_launch_metadata'
        ? 'runtime proof'
        : 'config proof',
  ].filter((value): value is string => Boolean(value));
}

function parseLabels(value: string): { readonly ok: true; readonly value: Record<string, string> } | { readonly ok: false; readonly error: string } {
  const labels: Record<string, string> = {};
  for (const entry of splitList(value)) {
    const separator = entry.indexOf('=');
    if (separator <= 0) {
      return { ok: false, error: 'Labels must use key=value format.' };
    }
    const key = entry.slice(0, separator).trim();
    const labelValue = entry.slice(separator + 1).trim();
    if (!key || !labelValue) {
      return { ok: false, error: 'Labels must include a non-empty key and value.' };
    }
    labels[key] = labelValue;
  }
  return { ok: true, value: labels };
}

function splitList(value: string): string[] {
  return value
    .split(/[\n,]/)
    .map((entry) => entry.trim())
    .filter(Boolean);
}
