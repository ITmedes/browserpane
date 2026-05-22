import type {
  BrowserContextListResponse,
  BrowserContextPersistenceMode,
  BrowserContextResource,
  BrowserContextState,
  BrowserContextUsageResource,
  EgressCustomCaConfig,
  EgressDiagnosticsHealth,
  EgressDiagnosticsProofLevel,
  EgressDiagnosticsResource,
  EgressProfileEffectiveStatus,
  EgressProfileListResponse,
  EgressProfileResource,
  EgressProfileState,
  EgressProxyConfig,
  EgressTrafficObservationConfig,
  EgressTrafficObservationMode,
  SessionAutomationDelegate,
  SessionBrowserContextMode,
  SessionBrowserContextResource,
  SessionConnectionCounts,
  SessionConnectInfo,
  SessionEffectiveEgress,
  SessionGeolocation,
  SessionAccessTokenResponse,
  SessionListResponse,
  SessionNetworkIdentity,
  SessionResource,
  SessionRuntimeInfo,
  SessionStatusSummary,
  SessionStopBlocker,
  SessionStopEligibility,
  SessionTemplateDefaults,
  SessionTemplateListResponse,
  SessionTemplateResource,
  SessionViewport,
} from './control-types';
import {
  expectBoolean,
  expectNumber,
  expectRecord,
  expectString,
  expectStringRecord,
  optionalString,
} from './control-wire';

export class ControlSessionMapper {
  static toEgressProfileList(payload: unknown): EgressProfileListResponse {
    const object = expectRecord(payload, 'egress profile list response');
    const profiles = object.profiles;
    if (!Array.isArray(profiles)) {
      throw new Error('egress profile list response must contain a profiles array');
    }
    return {
      profiles: profiles.map((profile) => this.toEgressProfileResource(profile)),
    };
  }

  static toEgressProfileResource(payload: unknown): EgressProfileResource {
    const object = expectRecord(payload, 'egress profile resource');
    const description = optionalString(object.description, 'egress profile description');
    return {
      id: expectString(object.id, 'egress profile id'),
      name: expectString(object.name, 'egress profile name'),
      description: description ?? null,
      labels: expectStringRecord(object.labels ?? {}, 'egress profile labels'),
      proxy: toEgressProxyConfig(object.proxy) ?? null,
      bypass_rules: toStringArray(object.bypass_rules ?? [], 'egress profile bypass_rules'),
      custom_ca: toEgressCustomCaConfig(object.custom_ca) ?? null,
      traffic_observation: toEgressTrafficObservationConfig(object.traffic_observation),
      state: expectEnum(object.state, 'egress profile state', EGRESS_PROFILE_STATES),
      effective: toEgressEffectiveStatus(object.effective),
      diagnostics: toEgressDiagnosticsResource(object.diagnostics),
      created_at: expectString(object.created_at, 'egress profile created_at'),
      updated_at: expectString(object.updated_at, 'egress profile updated_at'),
    };
  }

  static toSessionNetworkIdentity(payload: unknown): SessionNetworkIdentity {
    return toSessionNetworkIdentity(payload);
  }

  static toSessionEffectiveEgress(payload: unknown): SessionEffectiveEgress {
    return toSessionEffectiveEgress(payload);
  }

  static toEgressDiagnosticsResource(payload: unknown): EgressDiagnosticsResource {
    return toEgressDiagnosticsResource(payload);
  }

  static toBrowserContextList(payload: unknown): BrowserContextListResponse {
    const object = expectRecord(payload, 'browser context list response');
    const contexts = object.contexts;
    if (!Array.isArray(contexts)) {
      throw new Error('browser context list response must contain a contexts array');
    }
    return {
      contexts: contexts.map((context) => this.toBrowserContextResource(context)),
    };
  }

  static toBrowserContextResource(payload: unknown): BrowserContextResource {
    const object = expectRecord(payload, 'browser context resource');
    const description = optionalString(object.description, 'browser context description');
    const lastUsedAt = optionalString(object.last_used_at, 'browser context last_used_at');
    const deletedAt = optionalString(object.deleted_at, 'browser context deleted_at');
    const retentionExpiresAt = optionalString(
      object.retention_expires_at,
      'browser context retention_expires_at',
    );
    const usage = toBrowserContextUsage(object.usage);
    return {
      id: expectString(object.id, 'browser context id'),
      name: expectString(object.name, 'browser context name'),
      description: description ?? null,
      labels: expectStringRecord(object.labels ?? {}, 'browser context labels'),
      persistence_mode: expectEnum(
        object.persistence_mode,
        'browser context persistence_mode',
        BROWSER_CONTEXT_PERSISTENCE_MODES,
      ),
      retention_sec: optionalNumber(object.retention_sec, 'browser context retention_sec') ?? null,
      retention_expires_at: retentionExpiresAt ?? null,
      max_profile_storage_bytes: optionalNumber(
        object.max_profile_storage_bytes,
        'browser context max_profile_storage_bytes',
      ) ?? null,
      state: expectEnum(object.state, 'browser context state', BROWSER_CONTEXT_STATES),
      usage: usage ?? null,
      created_at: expectString(object.created_at, 'browser context created_at'),
      updated_at: expectString(object.updated_at, 'browser context updated_at'),
      last_used_at: lastUsedAt ?? null,
      deleted_at: deletedAt ?? null,
    };
  }

  static toSessionList(payload: unknown): SessionListResponse {
    const object = expectRecord(payload, 'session list response');
    const sessions = object.sessions;
    if (!Array.isArray(sessions)) {
      throw new Error('session list response must contain a sessions array');
    }
    return {
      sessions: sessions.map((session) => this.toSessionResource(session)),
    };
  }

  static toSessionTemplateList(payload: unknown): SessionTemplateListResponse {
    const object = expectRecord(payload, 'session template list response');
    const templates = object.templates;
    if (!Array.isArray(templates)) {
      throw new Error('session template list response must contain a templates array');
    }
    return {
      templates: templates.map((template) => this.toSessionTemplateResource(template)),
    };
  }

  static toSessionResource(payload: unknown): SessionResource {
    const object = expectRecord(payload, 'session resource');
    const templateId = optionalString(object.template_id, 'session resource template_id');
    const stoppedAt = optionalString(object.stopped_at, 'session resource stopped_at');
    const runtimeReleasedAt = optionalString(
      object.runtime_released_at,
      'session resource runtime_released_at',
    );
    const automationDelegate = toAutomationDelegate(object.automation_delegate);
    return {
      id: expectString(object.id, 'session resource id'),
      state: expectString(object.state, 'session resource state'),
      template_id: templateId ?? null,
      browser_context: toSessionBrowserContextResource(object.browser_context),
      network_identity: toSessionNetworkIdentity(object.network_identity),
      effective_egress: toSessionEffectiveEgress(object.effective_egress),
      egress_diagnostics: toEgressDiagnosticsResource(object.egress_diagnostics),
      owner_mode: expectString(object.owner_mode, 'session resource owner_mode'),
      viewport: toOptionalViewport(object.viewport, 'session resource viewport') ?? null,
      idle_timeout_sec: optionalNumber(object.idle_timeout_sec, 'session resource idle_timeout_sec') ?? null,
      labels: expectStringRecord(object.labels ?? {}, 'session resource labels'),
      integration_context: optionalRecord(object.integration_context, 'session resource integration_context') ?? null,
      ...(automationDelegate !== undefined ? { automation_delegate: automationDelegate } : {}),
      connect: toConnectInfo(object.connect),
      runtime: toRuntimeInfo(object.runtime),
      status: toStatusSummary(object.status),
      created_at: expectString(object.created_at, 'session resource created_at'),
      updated_at: expectString(object.updated_at, 'session resource updated_at'),
      ...(runtimeReleasedAt !== undefined ? { runtime_released_at: runtimeReleasedAt } : {}),
      ...(stoppedAt !== undefined ? { stopped_at: stoppedAt } : {}),
    };
  }

  static toSessionTemplateResource(payload: unknown): SessionTemplateResource {
    const object = expectRecord(payload, 'session template resource');
    const description = optionalString(object.description, 'session template description');
    return {
      id: expectString(object.id, 'session template id'),
      name: expectString(object.name, 'session template name'),
      description: description ?? null,
      labels: expectStringRecord(object.labels ?? {}, 'session template labels'),
      defaults: toSessionTemplateDefaults(object.defaults ?? {}),
      version: expectNumber(object.version, 'session template version'),
      created_at: expectString(object.created_at, 'session template created_at'),
      updated_at: expectString(object.updated_at, 'session template updated_at'),
    };
  }

  static toSessionAccessTokenResponse(payload: unknown): SessionAccessTokenResponse {
    const object = expectRecord(payload, 'session access token response');
    return {
      session_id: expectString(object.session_id, 'session access token session_id'),
      token_type: expectString(object.token_type, 'session access token token_type'),
      token: expectString(object.token, 'session access token token'),
      expires_at: expectString(object.expires_at, 'session access token expires_at'),
      connect: toConnectInfo(object.connect),
    };
  }
}

const BROWSER_CONTEXT_STATES = ['ready', 'deleted'] satisfies readonly BrowserContextState[];
const BROWSER_CONTEXT_PERSISTENCE_MODES = ['reusable', 'ephemeral'] satisfies readonly BrowserContextPersistenceMode[];
const SESSION_BROWSER_CONTEXT_MODES = ['fresh', 'ephemeral', 'reusable'] satisfies readonly SessionBrowserContextMode[];
const EGRESS_PROFILE_STATES = ['ready', 'disabled'] satisfies readonly EgressProfileState[];
const EGRESS_TRAFFIC_OBSERVATION_MODES = ['metadata_only', 'tls_intercept'] satisfies readonly EgressTrafficObservationMode[];
const EGRESS_DIAGNOSTICS_HEALTHS = ['ready', 'unknown', 'attention', 'blocked', 'missing'] satisfies readonly EgressDiagnosticsHealth[];
const EGRESS_DIAGNOSTICS_PROOF_LEVELS = ['none', 'configuration', 'runtime_launch_metadata', 'active_probe'] satisfies readonly EgressDiagnosticsProofLevel[];

function expectEnum<T extends string>(
  value: unknown,
  label: string,
  allowed: readonly T[],
): T {
  const stringValue = expectString(value, label);
  if (!allowed.includes(stringValue as T)) {
    throw new Error(`${label} must be one of ${allowed.join(', ')}`);
  }
  return stringValue as T;
}

function optionalNumber(value: unknown, label: string): number | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return expectNumber(value, label);
}

function optionalRecord(value: unknown, label: string): Readonly<Record<string, unknown>> | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return expectRecord(value, label);
}

function toStringArray(value: unknown, label: string): readonly string[] {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array`);
  }
  return value.map((entry, index) => expectString(entry, `${label}[${index}]`));
}

function toEgressProxyConfig(value: unknown): EgressProxyConfig | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  const object = expectRecord(value, 'egress profile proxy');
  return {
    url: expectString(object.url, 'egress profile proxy url'),
  };
}

function toEgressCustomCaConfig(value: unknown): EgressCustomCaConfig | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  const object = expectRecord(value, 'egress profile custom_ca');
  const displayName = optionalString(object.display_name, 'egress profile custom_ca display_name');
  return {
    certificate_ref: expectString(
      object.certificate_ref,
      'egress profile custom_ca certificate_ref',
    ),
    display_name: displayName ?? null,
  };
}

function toEgressTrafficObservationConfig(value: unknown): EgressTrafficObservationConfig {
  const object = value === undefined || value === null
    ? {}
    : expectRecord(value, 'egress profile traffic_observation');
  const sinkRef = optionalString(
    object.sensitive_log_sink_ref,
    'egress profile traffic_observation sensitive_log_sink_ref',
  );
  const sinkName = optionalString(
    object.sensitive_log_sink_display_name,
    'egress profile traffic_observation sensitive_log_sink_display_name',
  );
  return {
    mode: object.mode === undefined || object.mode === null
      ? 'metadata_only'
      : expectEnum(
        object.mode,
        'egress profile traffic_observation mode',
        EGRESS_TRAFFIC_OBSERVATION_MODES,
      ),
    sensitive_log_sink_ref: sinkRef ?? null,
    sensitive_log_sink_display_name: sinkName ?? null,
  };
}

function toEgressEffectiveStatus(value: unknown): EgressProfileEffectiveStatus {
  const object = value === undefined || value === null
    ? {}
    : expectRecord(value, 'egress profile effective');
  return {
    proxy_configured: expectBoolean(
      object.proxy_configured ?? false,
      'egress profile effective proxy_configured',
    ),
    bypass_rule_count: expectNumber(
      object.bypass_rule_count ?? 0,
      'egress profile effective bypass_rule_count',
    ),
    custom_ca_configured: expectBoolean(
      object.custom_ca_configured ?? false,
      'egress profile effective custom_ca_configured',
    ),
    observation_mode: object.observation_mode === undefined || object.observation_mode === null
      ? 'metadata_only'
      : expectEnum(
        object.observation_mode,
        'egress profile effective observation_mode',
        EGRESS_TRAFFIC_OBSERVATION_MODES,
      ),
    tls_interception_enabled: expectBoolean(
      object.tls_interception_enabled ?? false,
      'egress profile effective tls_interception_enabled',
    ),
    sensitive_log_sink_configured: expectBoolean(
      object.sensitive_log_sink_configured ?? false,
      'egress profile effective sensitive_log_sink_configured',
    ),
  };
}

function toEgressDiagnosticsResource(value: unknown): EgressDiagnosticsResource {
  const object = value === undefined || value === null
    ? {}
    : expectRecord(value, 'egress diagnostics');
  const profileId = optionalString(object.profile_id, 'egress diagnostics profile_id');
  const profileName = optionalString(object.profile_name, 'egress diagnostics profile_name');
  const runtimeBinding = optionalString(object.runtime_binding, 'egress diagnostics runtime_binding');
  const runtimeAssignment = optionalString(object.runtime_assignment, 'egress diagnostics runtime_assignment');
  const proof = object.proof === undefined || object.proof === null
    ? {}
    : expectRecord(object.proof, 'egress diagnostics proof');
  const observedPublicIp = optionalString(proof.observed_public_ip, 'egress diagnostics proof observed_public_ip');
  const observedTlsIssuer = optionalString(proof.observed_tls_issuer, 'egress diagnostics proof observed_tls_issuer');
  const lastFailureReason = optionalString(proof.last_failure_reason, 'egress diagnostics proof last_failure_reason');
  const profileReachabilityObservedAt = optionalString(
    proof.profile_reachability_observed_at,
    'egress diagnostics proof profile_reachability_observed_at',
  );
  const profileReachabilityFailure = optionalString(
    proof.profile_reachability_failure,
    'egress diagnostics proof profile_reachability_failure',
  );
  return {
    profile_id: profileId ?? null,
    profile_name: profileName ?? null,
    profile_state: object.profile_state === undefined || object.profile_state === null
      ? null
      : expectEnum(object.profile_state, 'egress diagnostics profile_state', EGRESS_PROFILE_STATES),
    health: object.health === undefined || object.health === null
      ? 'unknown'
      : expectEnum(object.health, 'egress diagnostics health', EGRESS_DIAGNOSTICS_HEALTHS),
    observation_mode: object.observation_mode === undefined || object.observation_mode === null
      ? 'metadata_only'
      : expectEnum(
        object.observation_mode,
        'egress diagnostics observation_mode',
        EGRESS_TRAFFIC_OBSERVATION_MODES,
      ),
    proof_level: object.proof_level === undefined || object.proof_level === null
      ? 'none'
      : expectEnum(
        object.proof_level,
        'egress diagnostics proof_level',
        EGRESS_DIAGNOSTICS_PROOF_LEVELS,
      ),
    runtime_binding: runtimeBinding ?? null,
    runtime_assignment: runtimeAssignment ?? null,
    proxy_configured: expectBoolean(object.proxy_configured ?? false, 'egress diagnostics proxy_configured'),
    bypass_rule_count: expectNumber(object.bypass_rule_count ?? 0, 'egress diagnostics bypass_rule_count'),
    custom_ca_configured: expectBoolean(object.custom_ca_configured ?? false, 'egress diagnostics custom_ca_configured'),
    tls_interception_enabled: expectBoolean(
      object.tls_interception_enabled ?? false,
      'egress diagnostics tls_interception_enabled',
    ),
    sensitive_log_sink_configured: expectBoolean(
      object.sensitive_log_sink_configured ?? false,
      'egress diagnostics sensitive_log_sink_configured',
    ),
    proof: {
      profile_resolved: expectBoolean(proof.profile_resolved ?? false, 'egress diagnostics proof profile_resolved'),
      profile_ready: expectBoolean(proof.profile_ready ?? false, 'egress diagnostics proof profile_ready'),
      profile_reachability_collected: expectBoolean(
        proof.profile_reachability_collected ?? false,
        'egress diagnostics proof profile_reachability_collected',
      ),
      profile_reachability_healthy: expectBoolean(
        proof.profile_reachability_healthy ?? false,
        'egress diagnostics proof profile_reachability_healthy',
      ),
      profile_reachability_observed_at: profileReachabilityObservedAt ?? null,
      profile_reachability_failure: profileReachabilityFailure ?? null,
      proxy_launch_config_expected: expectBoolean(
        proof.proxy_launch_config_expected ?? false,
        'egress diagnostics proof proxy_launch_config_expected',
      ),
      bypass_rules_expected: expectNumber(
        proof.bypass_rules_expected ?? 0,
        'egress diagnostics proof bypass_rules_expected',
      ),
      custom_ca_launch_config_expected: expectBoolean(
        proof.custom_ca_launch_config_expected ?? false,
        'egress diagnostics proof custom_ca_launch_config_expected',
      ),
      tls_interception_expected: expectBoolean(
        proof.tls_interception_expected ?? false,
        'egress diagnostics proof tls_interception_expected',
      ),
      sensitive_log_sink_declared: expectBoolean(
        proof.sensitive_log_sink_declared ?? false,
        'egress diagnostics proof sensitive_log_sink_declared',
      ),
      runtime_launch_observed: expectBoolean(
        proof.runtime_launch_observed ?? false,
        'egress diagnostics proof runtime_launch_observed',
      ),
      active_probe_collected: expectBoolean(
        proof.active_probe_collected ?? false,
        'egress diagnostics proof active_probe_collected',
      ),
      observed_public_ip: observedPublicIp ?? null,
      observed_tls_issuer: observedTlsIssuer ?? null,
      last_failure_reason: lastFailureReason ?? null,
    },
    warnings: toStringArray(object.warnings ?? [], 'egress diagnostics warnings'),
    observed_at: optionalString(object.observed_at, 'egress diagnostics observed_at') ?? '',
  };
}

function toOptionalSessionNetworkIdentity(
  value: unknown,
  label: string,
): SessionNetworkIdentity | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return toSessionNetworkIdentity(value, label);
}

function toSessionNetworkIdentity(
  value: unknown,
  label = 'session network_identity',
): SessionNetworkIdentity {
  const object = value === undefined || value === null ? {} : expectRecord(value, label);
  const locale = optionalString(object.locale, `${label} locale`);
  const timezone = optionalString(object.timezone, `${label} timezone`);
  const userAgent = optionalString(object.user_agent, `${label} user_agent`);
  const browserIdentity = optionalString(object.browser_identity, `${label} browser_identity`);
  const egressProfileId = optionalString(object.egress_profile_id, `${label} egress_profile_id`);
  return {
    locale: locale ?? null,
    languages: toStringArray(object.languages ?? [], `${label} languages`),
    timezone: timezone ?? null,
    geolocation: toSessionGeolocation(object.geolocation, `${label} geolocation`) ?? null,
    user_agent: userAgent ?? null,
    browser_identity: browserIdentity ?? null,
    egress_profile_id: egressProfileId ?? null,
  };
}

function toSessionGeolocation(
  value: unknown,
  label: string,
): SessionGeolocation | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  const object = expectRecord(value, label);
  return {
    latitude: expectNumber(object.latitude, `${label} latitude`),
    longitude: expectNumber(object.longitude, `${label} longitude`),
    accuracy_meters: optionalNumber(object.accuracy_meters, `${label} accuracy_meters`) ?? null,
  };
}

function toSessionEffectiveEgress(value: unknown): SessionEffectiveEgress {
  const object = value === undefined || value === null
    ? {}
    : expectRecord(value, 'session effective_egress');
  const profileId = optionalString(object.profile_id, 'session effective_egress profile_id');
  const profileName = optionalString(object.profile_name, 'session effective_egress profile_name');
  return {
    profile_id: profileId ?? null,
    profile_name: profileName ?? null,
    profile_state: object.profile_state === undefined || object.profile_state === null
      ? null
      : expectEnum(object.profile_state, 'session effective_egress profile_state', EGRESS_PROFILE_STATES),
    proxy_configured: expectBoolean(
      object.proxy_configured ?? false,
      'session effective_egress proxy_configured',
    ),
    bypass_rule_count: expectNumber(
      object.bypass_rule_count ?? 0,
      'session effective_egress bypass_rule_count',
    ),
    custom_ca_configured: expectBoolean(
      object.custom_ca_configured ?? false,
      'session effective_egress custom_ca_configured',
    ),
    observation_mode: object.observation_mode === undefined || object.observation_mode === null
      ? 'metadata_only'
      : expectEnum(
        object.observation_mode,
        'session effective_egress observation_mode',
        EGRESS_TRAFFIC_OBSERVATION_MODES,
      ),
    tls_interception_enabled: expectBoolean(
      object.tls_interception_enabled ?? false,
      'session effective_egress tls_interception_enabled',
    ),
    sensitive_log_sink_configured: expectBoolean(
      object.sensitive_log_sink_configured ?? false,
      'session effective_egress sensitive_log_sink_configured',
    ),
  };
}

function toBrowserContextUsage(value: unknown): BrowserContextUsageResource | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  const object = expectRecord(value, 'browser context usage');
  const activeRuntimeSessionId = optionalString(
    object.active_runtime_session_id,
    'browser context usage active_runtime_session_id',
  );
  const profileStorageBytes = optionalNumber(
    object.profile_storage_bytes,
    'browser context usage profile_storage_bytes',
  );
  return {
    visible_session_count: expectNumber(
      object.visible_session_count,
      'browser context usage visible_session_count',
    ),
    active_runtime_session_count: expectNumber(
      object.active_runtime_session_count,
      'browser context usage active_runtime_session_count',
    ),
    active_runtime_session_id: activeRuntimeSessionId ?? null,
    profile_storage_bytes: profileStorageBytes ?? null,
    profile_storage_limit_exceeded: expectBoolean(
      object.profile_storage_limit_exceeded ?? false,
      'browser context usage profile_storage_limit_exceeded',
    ),
  };
}

function toOptionalViewport(value: unknown, label: string): SessionViewport | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  const object = expectRecord(value, label);
  return {
    width: expectNumber(object.width, `${label} width`),
    height: expectNumber(object.height, `${label} height`),
  };
}

function toSessionTemplateDefaults(value: unknown): SessionTemplateDefaults {
  const object = expectRecord(value, 'session template defaults');
  const ownerMode = optionalString(object.owner_mode, 'session template defaults owner_mode');
  return {
    ...(ownerMode !== undefined ? { owner_mode: ownerMode } : {}),
    viewport: toOptionalViewport(object.viewport, 'session template defaults viewport') ?? null,
    idle_timeout_sec: optionalNumber(
      object.idle_timeout_sec,
      'session template defaults idle_timeout_sec',
    ) ?? null,
    labels: expectStringRecord(object.labels ?? {}, 'session template defaults labels'),
    integration_context: optionalRecord(
      object.integration_context,
      'session template defaults integration_context',
    ) ?? null,
    recording: optionalRecord(object.recording, 'session template defaults recording') ?? null,
    network_identity: toOptionalSessionNetworkIdentity(
      object.network_identity,
      'session template defaults network_identity',
    ) ?? null,
  };
}

function toConnectInfo(value: unknown): SessionConnectInfo {
  const object = expectRecord(value, 'session resource connect');
  const ticketPath = optionalString(object.ticket_path, 'session connect ticket_path');
  return {
    gateway_url: expectString(object.gateway_url, 'session connect gateway_url'),
    transport_path: expectString(object.transport_path, 'session connect transport_path'),
    auth_type: expectString(object.auth_type, 'session connect auth_type'),
    ...(ticketPath !== undefined ? { ticket_path: ticketPath } : {}),
    compatibility_mode: expectString(object.compatibility_mode, 'session connect compatibility_mode'),
  };
}

function toSessionBrowserContextResource(value: unknown): SessionBrowserContextResource {
  if (value === undefined || value === null) {
    return { mode: 'fresh', context_id: null };
  }
  const object = expectRecord(value, 'session resource browser_context');
  const contextId = optionalString(
    object.context_id,
    'session resource browser_context context_id',
  );
  return {
    mode: expectEnum(
      object.mode,
      'session resource browser_context mode',
      SESSION_BROWSER_CONTEXT_MODES,
    ),
    context_id: contextId ?? null,
  };
}

function toAutomationDelegate(value: unknown): SessionAutomationDelegate | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  const object = expectRecord(value, 'session resource automation_delegate');
  const displayName = optionalString(
    object.display_name,
    'session automation_delegate display_name',
  );
  return {
    client_id: expectString(object.client_id, 'session automation_delegate client_id'),
    issuer: expectString(object.issuer, 'session automation_delegate issuer'),
    ...(displayName !== undefined ? { display_name: displayName } : {}),
  };
}

function toRuntimeInfo(value: unknown): SessionRuntimeInfo {
  const object = expectRecord(value, 'session resource runtime');
  const cdpEndpoint = optionalString(object.cdp_endpoint, 'session runtime cdp_endpoint');
  return {
    binding: expectString(object.binding, 'session runtime binding'),
    compatibility_mode: expectString(object.compatibility_mode, 'session runtime compatibility_mode'),
    ...(cdpEndpoint !== undefined ? { cdp_endpoint: cdpEndpoint } : {}),
  };
}

function toStatusSummary(value: unknown): SessionStatusSummary {
  const object = expectRecord(value, 'session resource status');
  return {
    runtime_state: expectString(object.runtime_state, 'session status runtime_state'),
    runtime_resume_mode: expectString(object.runtime_resume_mode, 'session status runtime_resume_mode'),
    presence_state: expectString(object.presence_state, 'session status presence_state'),
    connection_counts: toConnectionCounts(object.connection_counts),
    stop_eligibility: toStopEligibility(object.stop_eligibility),
  };
}

function toConnectionCounts(value: unknown): SessionConnectionCounts {
  const object = expectRecord(value, 'session status connection_counts');
  return {
    interactive_clients: expectNumber(object.interactive_clients, 'interactive_clients'),
    owner_clients: expectNumber(object.owner_clients, 'owner_clients'),
    viewer_clients: expectNumber(object.viewer_clients, 'viewer_clients'),
    recorder_clients: expectNumber(object.recorder_clients, 'recorder_clients'),
    automation_clients: expectNumber(object.automation_clients, 'automation_clients'),
    total_clients: expectNumber(object.total_clients, 'total_clients'),
  };
}

function toStopEligibility(value: unknown): SessionStopEligibility {
  const object = expectRecord(value, 'session status stop_eligibility');
  const blockers = object.blockers;
  if (!Array.isArray(blockers)) {
    throw new Error('session stop eligibility blockers must be an array');
  }
  return {
    allowed: expectBoolean(object.allowed, 'session stop eligibility allowed'),
    blockers: blockers.map((blocker) => toStopBlocker(blocker)),
  };
}

function toStopBlocker(value: unknown): SessionStopBlocker {
  const object = expectRecord(value, 'session stop blocker');
  return {
    kind: expectString(object.kind, 'session stop blocker kind'),
    count: expectNumber(object.count, 'session stop blocker count'),
  };
}
