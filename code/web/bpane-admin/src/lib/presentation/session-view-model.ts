import type {
  BrowserContextResource,
  EgressDiagnosticsResource,
  SessionEffectiveEgress,
  SessionNetworkIdentity,
  SessionResource,
  SessionStopEligibility,
  SessionTemplateResource,
} from '../api/control-types';
import type { SessionStatus } from '../api/session-status-types';

export type SessionListItemViewModel = {
  readonly id: string;
  readonly shortId: string;
  readonly lifecycle: string;
  readonly runtime: string;
  readonly presence: string;
  readonly clients: number;
  readonly updatedAt: string;
  readonly template: string;
  readonly templateId: string | null;
  readonly browserContext: string;
  readonly browserContextId: string | null;
  readonly networkIdentity: string;
  readonly egress: string;
  readonly egressDiagnostics: string;
  readonly mcpDelegation: string;
  readonly labels: string;
};

export type SelectedSessionViewModel = SessionListItemViewModel & {
  readonly ownerMode: string;
  readonly runtimeBinding: string;
  readonly canJoin: boolean;
};

export type SessionListPanelViewModel = {
  readonly sessions: readonly SessionListItemViewModel[];
  readonly selectedSession: SelectedSessionViewModel | null;
  readonly selectedSessionId: string | null;
  readonly authenticated: boolean;
  readonly loading: boolean;
  readonly error: string | null;
};

export type SessionFactViewModel = {
  readonly label: string;
  readonly value: string;
  readonly testId?: string;
};

export type SessionConnectionViewModel = {
  readonly id: number;
  readonly label: string;
  readonly role: string;
  readonly canDisconnect: boolean;
};

export type SessionDetailPanelViewModel = {
  readonly title: string;
  readonly facts: readonly SessionFactViewModel[];
  readonly connections: readonly SessionConnectionViewModel[];
  readonly hint: string | null;
  readonly statusHint: string | null;
  readonly canRefresh: boolean;
  readonly canStop: boolean;
  readonly canKill: boolean;
  readonly canRelease: boolean;
  readonly canDisconnectAll: boolean;
  readonly loading: boolean;
  readonly error: string | null;
};

export class SessionViewModelBuilder {
  static list(input: {
    readonly sessions: readonly SessionResource[];
    readonly sessionTemplates?: readonly SessionTemplateResource[];
    readonly browserContexts?: readonly BrowserContextResource[];
    readonly selectedSessionId: string | null;
    readonly authenticated: boolean;
    readonly loading: boolean;
    readonly error: string | null;
  }): SessionListPanelViewModel {
    const selectedSession = input.sessions.find((session) => session.id === input.selectedSessionId) ?? null;
    const templateLookup = templateLookupFrom(input.sessionTemplates ?? []);
    const browserContextLookup = browserContextLookupFrom(input.browserContexts ?? []);
    return {
      sessions: input.sessions.map((session) => toListItem(session, templateLookup, browserContextLookup)),
      selectedSession: selectedSession ? toSelectedSession(selectedSession, templateLookup, browserContextLookup) : null,
      selectedSessionId: input.selectedSessionId,
      authenticated: input.authenticated,
      loading: input.loading,
      error: input.error,
    };
  }

  static detail(input: {
    readonly session: SessionResource | null;
    readonly sessionTemplates?: readonly SessionTemplateResource[];
    readonly browserContexts?: readonly BrowserContextResource[];
    readonly status?: SessionStatus | null;
    readonly connected: boolean;
    readonly loading: boolean;
    readonly error: string | null;
  }): SessionDetailPanelViewModel {
    const session = input.session;
    if (!session) {
      return {
        title: 'No session selected',
        facts: [],
        connections: [],
        hint: 'Select or create a session to inspect lifecycle and runtime state.',
        statusHint: null,
        canRefresh: false,
        canStop: false,
        canKill: false,
        canRelease: false,
        canDisconnectAll: false,
        loading: input.loading,
        error: input.error,
      };
    }
    const status = input.status ?? null;
    const templateLookup = templateLookupFrom(input.sessionTemplates ?? []);
    const browserContextLookup = browserContextLookupFrom(input.browserContexts ?? []);
    const stopEligibility = status?.stop_eligibility ?? session.status.stop_eligibility;
    const connectionCount = status?.connection_counts.total_clients
      ?? session.status.connection_counts.total_clients;
    const hasLiveClients = input.connected || connectionCount > 0;
    return {
      title: session.id,
      facts: [
        { label: 'state', value: session.state, testId: 'session-state' },
        {
          label: 'template',
          value: templateLabel(session, templateLookup),
          testId: 'session-template',
        },
        {
          label: 'browser context',
          value: browserContextLabel(session, browserContextLookup),
          testId: 'session-browser-context',
        },
        { label: 'owner', value: session.owner_mode, testId: 'session-owner-mode' },
        { label: 'idle override', value: session.idle_timeout_sec?.toString() ?? 'default', testId: 'session-idle-timeout' },
        {
          label: 'network identity',
          value: networkIdentityLabel(session.network_identity),
          testId: 'session-network-identity',
        },
        {
          label: 'egress',
          value: effectiveEgressLabel(session.effective_egress),
          testId: 'session-effective-egress',
        },
        {
          label: 'egress proof',
          value: egressDiagnosticsLabel(status?.egress_diagnostics ?? session.egress_diagnostics),
          testId: 'session-egress-diagnostics',
        },
        { label: 'labels', value: labelSummary(session.labels ?? {}), testId: 'session-labels' },
        {
          label: 'integration',
          value: integrationContextSummary(session.integration_context ?? null),
          testId: 'session-integration-context',
        },
        { label: 'runtime', value: session.status.runtime_state, testId: 'session-runtime-state' },
        { label: 'resume', value: session.status.runtime_resume_mode, testId: 'session-runtime-resume-mode' },
        { label: 'presence', value: session.status.presence_state, testId: 'session-presence-state' },
        { label: 'clients', value: String(connectionCount), testId: 'session-total-clients' },
        { label: 'binding', value: session.runtime.binding },
        { label: 'transport', value: session.connect.compatibility_mode },
        { label: 'created', value: session.created_at },
        { label: 'updated', value: session.updated_at },
        ...(session.runtime_released_at ? [{ label: 'runtime released', value: session.runtime_released_at }] : []),
        ...(session.stopped_at ? [{ label: 'stopped', value: session.stopped_at }] : []),
        ...statusFacts(status),
      ],
      connections: status?.connections.map((connection) => ({
        id: connection.connection_id,
        label: `#${connection.connection_id}`,
        role: connection.role,
        canDisconnect: !input.loading,
      })) ?? [],
      hint: resolveHint(hasLiveClients, stopEligibility),
      statusHint: status ? null : 'Live status is loaded from the session status API.',
      canRefresh: !input.loading,
      canStop: !input.loading && !hasLiveClients && stopEligibility.allowed,
      canKill: !input.loading && !hasLiveClients,
      canRelease: !input.loading && !hasLiveClients && stopEligibility.allowed && !['released', 'stopped'].includes(session.state),
      canDisconnectAll: !input.loading && (status?.connections.length ?? 0) > 0,
      loading: input.loading,
      error: input.error,
    };
  }
}

function toListItem(
  session: SessionResource,
  templates: ReadonlyMap<string, SessionTemplateResource>,
  browserContexts: ReadonlyMap<string, BrowserContextResource>,
): SessionListItemViewModel {
  return {
    id: session.id,
    shortId: shortId(session.id),
    lifecycle: session.state,
    runtime: session.status.runtime_state,
    presence: session.status.presence_state,
    clients: session.status.connection_counts.total_clients,
    updatedAt: session.updated_at,
    template: templateLabel(session, templates),
    templateId: session.template_id ?? null,
    browserContext: browserContextLabel(session, browserContexts),
    browserContextId: session.browser_context?.context_id ?? null,
    networkIdentity: networkIdentityLabel(session.network_identity),
    egress: effectiveEgressLabel(session.effective_egress),
    egressDiagnostics: egressDiagnosticsLabel(session.egress_diagnostics),
    mcpDelegation: mcpDelegationLabel(session),
    labels: labelSummary(session.labels ?? {}),
  };
}

function toSelectedSession(
  session: SessionResource,
  templates: ReadonlyMap<string, SessionTemplateResource>,
  browserContexts: ReadonlyMap<string, BrowserContextResource>,
): SelectedSessionViewModel {
  return {
    ...toListItem(session, templates, browserContexts),
    ownerMode: session.owner_mode,
    runtimeBinding: session.runtime.binding,
    canJoin: canConnectSession(session.state),
  };
}

function canConnectSession(state: string): boolean {
  return ['pending', 'starting', 'ready', 'active', 'idle', 'released', 'stopped'].includes(state);
}

function statusFacts(status: SessionStatus | null): SessionFactViewModel[] {
  if (!status) {
    return [];
  }
  return [
    { label: 'status state', value: status.state },
    { label: 'resolution', value: `${status.resolution[0]}x${status.resolution[1]}` },
    { label: 'mcp owner', value: yesNo(status.mcp_owner), testId: 'session-mcp-owner' },
    { label: 'exclusive owner', value: yesNo(status.exclusive_browser_owner) },
    { label: 'idle timeout', value: status.idle.idle_timeout_sec?.toString() ?? 'none' },
    { label: 'recording', value: status.recording.state, testId: 'session-recording-state' },
    { label: 'playback', value: `${status.playback.included_segment_count}/${status.playback.segment_count}` },
    { label: 'join latency avg', value: `${status.telemetry.average_join_latency_ms.toFixed(1)} ms` },
  ];
}

function resolveHint(connected: boolean, stopEligibility: SessionStopEligibility): string | null {
  if (connected) {
    return 'Disconnect the embedded browser before stopping this session.';
  }
  if (stopEligibility.allowed) {
    return null;
  }
  const blockers = stopEligibility.blockers;
  const reason = blockers.length === 0
    ? 'the current runtime state'
    : blockers.map((blocker) => `${blocker.count} ${blocker.kind}`).join(', ');
  return `Stop is blocked by ${reason}.`;
}

function yesNo(value: boolean): string {
  return value ? 'yes' : 'no';
}

function mcpDelegationLabel(session: SessionResource): string {
  return session.automation_delegate ? 'MCP delegated' : 'MCP not delegated';
}

function labelSummary(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels).sort(([left], [right]) => left.localeCompare(right));
  if (entries.length === 0) {
    return 'No labels';
  }
  return entries.map(([key, value]) => `${key}=${value}`).join(', ');
}

function networkIdentityLabel(identity: SessionNetworkIdentity | null | undefined): string {
  if (!identity) {
    return 'Default network identity';
  }
  const facts: string[] = [];
  if (identity.locale) {
    facts.push(identity.locale);
  }
  if (identity.timezone) {
    facts.push(identity.timezone);
  }
  if (identity.languages && identity.languages.length > 0) {
    facts.push(identity.languages.join('/'));
  }
  if (identity.geolocation) {
    facts.push(`geo ${identity.geolocation.latitude},${identity.geolocation.longitude}`);
  }
  if (identity.browser_identity) {
    facts.push(identity.browser_identity);
  } else if (identity.user_agent) {
    facts.push('custom user agent');
  }
  return facts.length > 0 ? facts.join(' | ') : 'Default network identity';
}

function effectiveEgressLabel(egress: SessionEffectiveEgress | null | undefined): string {
  if (!egress) {
    return 'Default egress';
  }
  if (!egress.profile_id) {
    return 'Default egress';
  }
  const facts = [
    egress.profile_name ?? `Profile ${shortId(egress.profile_id)}`,
    egress.profile_state ?? 'unknown',
    egress.proxy_configured ? 'proxy' : null,
    egress.tls_interception_enabled ? 'TLS inspect' : null,
    egress.sensitive_log_sink_configured ? 'log sink' : null,
    egress.custom_ca_configured ? 'custom CA' : null,
    egress.bypass_rule_count > 0 ? `${egress.bypass_rule_count} bypass` : null,
  ].filter(Boolean);
  return facts.join(' | ');
}

function egressDiagnosticsLabel(diagnostics: EgressDiagnosticsResource | null | undefined): string {
  if (!diagnostics) {
    return 'No egress diagnostics';
  }
  const facts = [
    diagnostics.health,
    diagnostics.proof_level.replaceAll('_', ' '),
    diagnostics.runtime_binding ? `runtime=${diagnostics.runtime_binding}` : null,
    diagnostics.runtime_assignment ? `assignment=${diagnostics.runtime_assignment}` : null,
    diagnostics.proof.runtime_launch_observed ? 'runtime launch observed' : null,
    diagnostics.proof.active_probe_collected
      ? 'active probe collected'
      : diagnostics.proof.last_failure_reason
        ? 'active probe failed'
        : 'active probe pending',
    diagnostics.proof.observed_public_ip ? `ip=${diagnostics.proof.observed_public_ip}` : null,
    diagnostics.proof.observed_tls_issuer ? `issuer=${diagnostics.proof.observed_tls_issuer}` : null,
    diagnostics.warnings.length > 0 ? `${diagnostics.warnings.length} warning` : null,
  ].filter(Boolean);
  return facts.join(' | ');
}

function integrationContextSummary(context: Readonly<Record<string, unknown>> | null): string {
  if (!context || Object.keys(context).length === 0) {
    return 'No integration context';
  }
  return Object.entries(context)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}=${formatContextValue(value)}`)
    .join(', ');
}

function formatContextValue(value: unknown): string {
  if (value === null) {
    return 'null';
  }
  if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }
  return JSON.stringify(value);
}

function templateLookupFrom(
  templates: readonly SessionTemplateResource[],
): ReadonlyMap<string, SessionTemplateResource> {
  return new Map(templates.map((template) => [template.id, template]));
}

function browserContextLookupFrom(
  browserContexts: readonly BrowserContextResource[],
): ReadonlyMap<string, BrowserContextResource> {
  return new Map(browserContexts.map((context) => [context.id, context]));
}

function templateLabel(
  session: SessionResource,
  templates: ReadonlyMap<string, SessionTemplateResource>,
): string {
  const templateId = session.template_id;
  if (!templateId) {
    return 'No template';
  }
  const template = templates.get(templateId);
  return template ? `${template.name} (${shortId(template.id)})` : `Template ${shortId(templateId)}`;
}

function browserContextLabel(
  session: SessionResource,
  contexts: ReadonlyMap<string, BrowserContextResource>,
): string {
  const browserContext = session.browser_context ?? { mode: 'fresh', context_id: null };
  if (browserContext.mode === 'fresh') {
    return 'Fresh profile';
  }
  if (browserContext.mode === 'ephemeral') {
    return 'Ephemeral profile';
  }
  const contextId = browserContext.context_id;
  if (!contextId) {
    return 'Reusable context without id';
  }
  const context = contexts.get(contextId);
  if (!context) {
    return `Context ${shortId(contextId)}`;
  }
  const stateSuffix = context.state === 'ready' ? '' : `, ${context.state}`;
  return `${context.name} (${shortId(context.id)}${stateSuffix})`;
}

function shortId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}
