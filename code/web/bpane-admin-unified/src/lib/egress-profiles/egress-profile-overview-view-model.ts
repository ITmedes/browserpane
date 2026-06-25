import { formatDateTime, type ProjectTone } from '$lib/projects/project-formatters';
import type {
  EgressDiagnosticsHealth,
  EgressDiagnosticsProofLevel,
  EgressProfileResource,
} from './egress-profile-types';

export type EgressProfileOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | {
      readonly status: 'ready';
      readonly profiles: readonly EgressProfileResource[];
    };

export type EgressProfileKind = 'direct' | 'proxy' | 'tls';

export type EgressProfileOverviewMetric = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type EgressProfileOverviewRow = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly state: string;
  readonly stateTone: ProjectTone;
  readonly kind: EgressProfileKind;
  readonly kindLabel: string;
  readonly kindTone: ProjectTone;
  readonly scope: string;
  readonly proxySummary: string;
  readonly observationSummary: string;
  readonly health: EgressDiagnosticsHealth;
  readonly healthLabel: string;
  readonly healthTone: ProjectTone;
  readonly proofLevel: EgressDiagnosticsProofLevel;
  readonly proofLabel: string;
  readonly badges: readonly string[];
  readonly updatedAt: string;
};

export type EgressProfileOverviewModel = {
  readonly metrics: readonly EgressProfileOverviewMetric[];
  readonly rows: readonly EgressProfileOverviewRow[];
};

export function buildEgressProfileOverviewModel(
  profiles: readonly EgressProfileResource[],
): EgressProfileOverviewModel {
  return {
    metrics: [
      metric('total', 'Profiles', profiles.length),
      metric('ready', 'Ready', profiles.filter((profile) => profile.state === 'ready').length),
      metric('tls', 'TLS intercept', profiles.filter((profile) => profile.effective.tls_interception_enabled).length),
      metric(
        'attention',
        'Needs attention',
        profiles.filter((profile) =>
          profile.state === 'disabled'
          || profile.diagnostics.health === 'attention'
          || profile.diagnostics.health === 'blocked'
          || profile.diagnostics.health === 'missing').length,
      ),
    ],
    rows: profiles.map(egressProfileRow),
  };
}
export function egressProfileRow(profile: EgressProfileResource): EgressProfileOverviewRow {
  const kind = egressProfileKind(profile);
  const health = profile.diagnostics.health;
  return {
    id: profile.id,
    name: profile.name,
    description: profile.description?.trim() || profile.id,
    state: profile.state,
    stateTone: profile.state === 'ready' ? 'success' : 'neutral',
    kind,
    kindLabel: egressProfileKindLabel(kind),
    kindTone: kind === 'tls' ? 'warning' : kind === 'proxy' ? 'success' : 'neutral',
    scope: profile.project?.name ?? profile.project_id ?? 'Owner scoped',
    proxySummary: egressProxySummary(profile),
    observationSummary: egressObservationSummary(profile),
    health,
    healthLabel: health.replaceAll('_', ' '),
    healthTone: egressHealthTone(health),
    proofLevel: profile.diagnostics.proof_level,
    proofLabel: egressProofLabel(profile.diagnostics.proof_level),
    badges: egressProfileBadges(profile),
    updatedAt: formatDateTime(profile.updated_at),
  };
}

export function egressProfileMatchesSearch(
  row: EgressProfileOverviewRow,
  normalizedQuery: string,
): boolean {
  if (!normalizedQuery) {
    return true;
  }
  return [
    row.id,
    row.name,
    row.description,
    row.state,
    row.kindLabel,
    row.scope,
    row.proxySummary,
    row.observationSummary,
    row.healthLabel,
    row.proofLabel,
    ...row.badges,
  ].some((value) => value.toLowerCase().includes(normalizedQuery));
}

function egressProfileKind(profile: EgressProfileResource): EgressProfileKind {
  if (profile.effective.tls_interception_enabled) {
    return 'tls';
  }
  if (profile.effective.proxy_configured) {
    return 'proxy';
  }
  return 'direct';
}

function egressProfileKindLabel(kind: EgressProfileKind): string {
  if (kind === 'tls') {
    return 'TLS interceptor';
  }
  if (kind === 'proxy') {
    return 'Egress proxy';
  }
  return 'Direct';
}

function egressProxySummary(profile: EgressProfileResource): string {
  if (!profile.proxy?.url) {
    return 'No proxy';
  }
  return [
    profile.proxy.url,
    profile.effective.proxy_auth_configured ? 'auth' : null,
    profile.effective.bypass_rule_count > 0 ? `${profile.effective.bypass_rule_count} bypass` : null,
  ].filter(Boolean).join(' · ');
}

function egressObservationSummary(profile: EgressProfileResource): string {
  if (profile.effective.tls_interception_enabled) {
    return profile.effective.sensitive_log_sink_configured
      ? 'TLS intercept with sensitive log sink'
      : 'TLS intercept';
  }
  if (profile.effective.custom_ca_configured) {
    return 'Custom CA configured';
  }
  return profile.effective.observation_mode === 'metadata_only'
    ? 'Metadata-only observation'
    : profile.effective.observation_mode;
}

function egressHealthTone(health: EgressDiagnosticsHealth): ProjectTone {
  if (health === 'ready') {
    return 'success';
  }
  if (health === 'attention' || health === 'unknown') {
    return 'warning';
  }
  return 'danger';
}

function egressProofLabel(proofLevel: EgressDiagnosticsProofLevel): string {
  if (proofLevel === 'active_probe') {
    return 'Active probe';
  }
  if (proofLevel === 'runtime_launch_metadata') {
    return 'Runtime proof';
  }
  if (proofLevel === 'configuration') {
    return 'Configuration proof';
  }
  return 'No proof';
}

function egressProfileBadges(profile: EgressProfileResource): readonly string[] {
  return [
    profile.effective.proxy_configured ? 'proxy' : 'direct',
    profile.effective.proxy_auth_configured ? 'proxy auth' : null,
    profile.effective.tls_interception_enabled ? 'TLS inspect' : null,
    profile.effective.custom_ca_configured ? 'custom CA' : null,
    profile.effective.sensitive_log_sink_configured ? 'log sink' : null,
    profile.project_id ? 'project scoped' : 'owner scoped',
    profile.state === 'disabled' ? 'disabled' : null,
    profile.diagnostics.health !== 'ready' ? `health ${profile.diagnostics.health}` : null,
    egressProofLabel(profile.diagnostics.proof_level),
  ].filter((value): value is string => Boolean(value));
}

function metric(key: string, label: string, value: number): EgressProfileOverviewMetric {
  return {
    label,
    value: String(value),
    testId: `egress-profiles-metric-${key}`,
  };
}
