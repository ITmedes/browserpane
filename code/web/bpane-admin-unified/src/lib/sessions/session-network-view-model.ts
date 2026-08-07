import type { EgressDiagnosticsResource } from '$lib/egress-profiles/egress-profile-types';
import { formatDateTime, type ProjectTone } from '$lib/projects/project-formatters';
import type { SessionResource } from './session-types';

export type SessionNetworkFact = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
  readonly tone?: ProjectTone;
};

export type SessionNetworkModel = {
  readonly mode: 'direct' | 'proxy' | 'tls_intercept';
  readonly modeLabel: string;
  readonly health: string;
  readonly healthTone: ProjectTone;
  readonly proofLevel: string;
  readonly profileLabel: string;
  readonly profileHref: string | null;
  readonly requestedFacts: readonly SessionNetworkFact[];
  readonly effectiveFacts: readonly SessionNetworkFact[];
  readonly proofFacts: readonly SessionNetworkFact[];
  readonly warnings: readonly string[];
  readonly canProbe: boolean;
  readonly probeBlockedReason: string | null;
};

export function buildSessionNetworkModel(
  session: SessionResource,
  diagnostics: EgressDiagnosticsResource | null,
): SessionNetworkModel {
  const requested = session.network_identity;
  const effective = session.effective_egress;
  const profileId = diagnostics?.profile_id ?? effective?.profile_id ?? requested?.egress_profile_id ?? null;
  const profileLabel = diagnostics?.profile_name ?? effective?.profile_name ?? profileId ?? 'No egress profile';
  const mode = diagnostics?.tls_interception_enabled || effective?.tls_interception_enabled
    ? 'tls_intercept'
    : diagnostics?.proxy_configured || effective?.proxy_configured
      ? 'proxy'
      : 'direct';
  const runtimeRunning = session.status.runtime_state === 'running';
  const assignmentReady = !diagnostics?.runtime_assignment || diagnostics.runtime_assignment === 'ready';
  const terminal = ['killed', 'cancelled', 'failed', 'stopped', 'released'].includes(session.state);
  const canProbe = runtimeRunning && assignmentReady && !terminal;

  return {
    mode,
    modeLabel: mode === 'tls_intercept' ? 'TLS interceptor' : mode === 'proxy' ? 'Forward proxy' : 'Direct egress',
    health: diagnostics?.health ?? 'unknown',
    healthTone: diagnosticsHealthTone(diagnostics?.health),
    proofLevel: diagnostics?.proof_level?.replaceAll('_', ' ') ?? 'none',
    profileLabel,
    profileHref: profileId ? `/admin-new/egress/${encodeURIComponent(profileId)}` : null,
    requestedFacts: [
      fact('Locale', requested?.locale ?? 'Default', 'session-network-requested-locale'),
      fact('Languages', requested?.languages?.join(', ') || 'Default', 'session-network-requested-languages'),
      fact('Timezone', requested?.timezone ?? 'Default', 'session-network-requested-timezone'),
      fact('Browser identity', requested?.browser_identity ?? 'Default', 'session-network-requested-browser'),
      fact('User agent', requested?.user_agent ?? 'Browser default', 'session-network-requested-user-agent'),
      fact('Egress profile', requested?.egress_profile_id ?? 'None', 'session-network-requested-profile'),
    ],
    effectiveFacts: [
      fact('Profile', profileLabel, 'session-network-effective-profile', profileStateTone(diagnostics?.profile_state ?? effective?.profile_state)),
      fact('Mode', mode === 'direct' ? 'Direct' : mode === 'proxy' ? 'Proxy' : 'Proxy with TLS interception', 'session-network-effective-mode'),
      fact('Proxy authentication', enabledLabel(diagnostics?.proxy_auth_configured ?? effective?.proxy_auth_configured), 'session-network-proxy-auth'),
      fact('Bypass rules', String(diagnostics?.bypass_rule_count ?? effective?.bypass_rule_count ?? 0), 'session-network-bypass-rules'),
      fact('Custom CA', enabledLabel(diagnostics?.custom_ca_configured ?? effective?.custom_ca_configured), 'session-network-custom-ca'),
      fact('Sensitive log sink', enabledLabel(diagnostics?.sensitive_log_sink_configured ?? effective?.sensitive_log_sink_configured), 'session-network-log-sink'),
    ],
    proofFacts: diagnostics ? [
      fact('Runtime binding', diagnostics.runtime_binding ?? 'Not assigned', 'session-network-runtime-binding'),
      fact('Runtime assignment', diagnostics.runtime_assignment ?? session.status.runtime_state, 'session-network-runtime-assignment'),
      fact('Profile reachability', diagnostics.proof.profile_reachability_collected
        ? diagnostics.proof.profile_reachability_healthy ? 'Healthy' : 'Failed'
        : 'Not collected', 'session-network-profile-reachability', diagnostics.proof.profile_reachability_collected
          ? diagnostics.proof.profile_reachability_healthy ? 'success' : 'danger'
          : 'neutral'),
      fact('Runtime launch', diagnostics.proof.runtime_launch_observed ? 'Observed' : 'Not observed', 'session-network-runtime-launch', diagnostics.proof.runtime_launch_observed ? 'success' : 'neutral'),
      fact('Browser probe', diagnostics.proof.active_probe_collected ? 'Collected' : 'Not collected', 'session-network-active-probe', diagnostics.proof.active_probe_collected ? 'success' : 'neutral'),
      fact('Public IP', diagnostics.proof.observed_public_ip ?? 'Not observed', 'session-network-public-ip'),
      fact('TLS issuer', diagnostics.proof.observed_tls_issuer ?? 'Not observed', 'session-network-tls-issuer'),
      fact('Observed', diagnostics.observed_at ? formatDateTime(diagnostics.observed_at) : 'Not reported', 'session-network-observed-at'),
    ] : [],
    warnings: diagnostics
      ? [
          ...diagnostics.warnings,
          diagnostics.proof.profile_reachability_failure,
          diagnostics.proof.last_failure_reason,
        ].filter((value): value is string => Boolean(value))
      : [],
    canProbe,
    probeBlockedReason: canProbe
      ? null
      : terminal || !runtimeRunning
        ? 'Start and connect the session before running an active egress probe.'
        : 'Wait until the assigned session runtime is ready before running an active egress probe.',
  };
}

function diagnosticsHealthTone(health: EgressDiagnosticsResource['health'] | undefined): ProjectTone {
  if (health === 'ready') {
    return 'success';
  }
  if (health === 'attention' || health === 'unknown') {
    return 'warning';
  }
  return health === 'blocked' || health === 'missing' ? 'danger' : 'neutral';
}

function profileStateTone(state: string | null | undefined): ProjectTone {
  if (state === 'ready') {
    return 'success';
  }
  return state === 'disabled' ? 'danger' : 'neutral';
}

function enabledLabel(value: boolean | null | undefined): string {
  return value ? 'Enabled' : 'Disabled';
}

function fact(
  label: string,
  value: string,
  testId: string,
  tone?: ProjectTone,
): SessionNetworkFact {
  return { label, value, testId, ...(tone ? { tone } : {}) };
}
