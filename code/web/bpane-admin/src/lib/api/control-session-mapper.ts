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
  IdentityAccessReviewResponse,
  IdentityDelegatedPrincipalResource,
  IdentityMappingKind,
  IdentityMappingListResponse,
  IdentityMappingResource,
  IdentityMappingReviewResource,
  IdentityMappingState,
  IdentityPrincipalResource,
  IdentityPrincipalType,
  IdentityResourceCounts,
  IdentityServicePrincipalReviewResource,
  IdentityUnmappedPrincipalSignalResource,
  ProjectAdmissionDecision,
  ProjectAdmissionReasonCode,
  ProjectAdmissionState,
  ProjectListResponse,
  ProjectQuotas,
  ProjectResource,
  ProjectState,
  ProjectUsageResource,
  ServicePrincipalListResponse,
  ServicePrincipalResource,
  ServicePrincipalState,
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
  SessionProjectResource,
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
  static toIdentityPrincipalResource(payload: unknown): IdentityPrincipalResource {
    return toIdentityPrincipalResource(payload);
  }

  static toIdentityAccessReview(payload: unknown): IdentityAccessReviewResponse {
    const object = expectRecord(payload, 'identity access review');
    const projects = object.projects;
    const identityMappings = object.identity_mappings;
    const unmappedPrincipalSignals = object.unmapped_principal_signals;
    const servicePrincipals = object.service_principals;
    const delegatedPrincipals = object.delegated_principals;
    if (!Array.isArray(projects)) {
      throw new Error('identity access review must contain a projects array');
    }
    if (!Array.isArray(servicePrincipals)) {
      throw new Error('identity access review must contain a service_principals array');
    }
    if (!Array.isArray(identityMappings)) {
      throw new Error('identity access review must contain an identity_mappings array');
    }
    if (!Array.isArray(unmappedPrincipalSignals)) {
      throw new Error('identity access review must contain an unmapped_principal_signals array');
    }
    if (!Array.isArray(delegatedPrincipals)) {
      throw new Error('identity access review must contain a delegated_principals array');
    }
    return {
      principal: toIdentityPrincipalResource(object.principal),
      generated_at: expectString(object.generated_at, 'identity access review generated_at'),
      projects: projects.map((project) => this.toProjectResource(project)),
      resource_counts: toIdentityResourceCounts(object.resource_counts),
      identity_mappings: identityMappings.map((mapping) => toIdentityMappingReview(mapping)),
      unmapped_principal_signals: unmappedPrincipalSignals.map((signal) => toIdentityUnmappedPrincipalSignal(signal)),
      service_principals: servicePrincipals.map((servicePrincipal) => toIdentityServicePrincipalReview(servicePrincipal)),
      delegated_principals: delegatedPrincipals.map((delegate) => toIdentityDelegatedPrincipal(delegate)),
    };
  }

  static toIdentityMappingList(payload: unknown): IdentityMappingListResponse {
    const object = expectRecord(payload, 'identity mapping list response');
    const mappings = object.identity_mappings;
    if (!Array.isArray(mappings)) {
      throw new Error('identity mapping list response must contain an identity_mappings array');
    }
    return {
      identity_mappings: mappings.map((mapping) => this.toIdentityMappingResource(mapping)),
    };
  }

  static toIdentityMappingResource(payload: unknown): IdentityMappingResource {
    return toIdentityMappingResource(payload);
  }

  static toServicePrincipalList(payload: unknown): ServicePrincipalListResponse {
    const object = expectRecord(payload, 'service principal list response');
    const servicePrincipals = object.service_principals;
    if (!Array.isArray(servicePrincipals)) {
      throw new Error('service principal list response must contain a service_principals array');
    }
    return {
      service_principals: servicePrincipals.map((servicePrincipal) => this.toServicePrincipalResource(servicePrincipal)),
    };
  }

  static toServicePrincipalResource(payload: unknown): ServicePrincipalResource {
    return toServicePrincipalResource(payload);
  }

  static toProjectList(payload: unknown): ProjectListResponse {
    const object = expectRecord(payload, 'project list response');
    const projects = object.projects;
    if (!Array.isArray(projects)) {
      throw new Error('project list response must contain a projects array');
    }
    return {
      projects: projects.map((project) => this.toProjectResource(project)),
    };
  }

  static toProjectResource(payload: unknown): ProjectResource {
    const object = expectRecord(payload, 'project resource');
    const description = optionalString(object.description, 'project description');
    return {
      id: expectString(object.id, 'project id'),
      name: expectString(object.name, 'project name'),
      description: description ?? null,
      labels: expectStringRecord(object.labels ?? {}, 'project labels'),
      quotas: toProjectQuotas(object.quotas),
      state: expectEnum(object.state, 'project state', PROJECT_STATES),
      usage: toProjectUsage(object.usage),
      created_at: expectString(object.created_at, 'project created_at'),
      updated_at: expectString(object.updated_at, 'project updated_at'),
    };
  }

  static toProjectUsage(payload: unknown): ProjectUsageResource {
    return toProjectUsage(payload);
  }

  static toProjectAdmissionDecision(payload: unknown): ProjectAdmissionDecision | null {
    return toProjectAdmissionDecision(payload);
  }

  static toSessionProjectResource(payload: unknown): SessionProjectResource | null {
    return toSessionProjectResource(payload) ?? null;
  }

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
      project_id: optionalString(object.project_id, 'session resource project_id') ?? null,
      project: toSessionProjectResource(object.project) ?? null,
      admission: toProjectAdmissionDecision(object.admission),
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
const PROJECT_STATES = ['active', 'archived'] satisfies readonly ProjectState[];
const PROJECT_ADMISSION_STATES = ['allowed', 'queued', 'rejected'] satisfies readonly ProjectAdmissionState[];
const PROJECT_ADMISSION_REASON_CODES = [
  'owner_scope_unbounded',
  'project_quota_available',
  'active_session_quota_exceeded',
  'project_archived',
] satisfies readonly ProjectAdmissionReasonCode[];
const IDENTITY_PRINCIPAL_TYPES = ['user', 'service_principal', 'legacy_dev_token'] satisfies readonly IdentityPrincipalType[];
const IDENTITY_MAPPING_KINDS = ['user', 'group', 'claim', 'service_principal'] satisfies readonly IdentityMappingKind[];
const IDENTITY_MAPPING_STATES = ['active', 'disabled'] satisfies readonly IdentityMappingState[];
const SERVICE_PRINCIPAL_STATES = ['active', 'disabled'] satisfies readonly ServicePrincipalState[];
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

function toIdentityPrincipalResource(value: unknown): IdentityPrincipalResource {
  const object = expectRecord(value, 'identity principal');
  const displayName = optionalString(object.display_name, 'identity principal display_name');
  const clientId = optionalString(object.client_id, 'identity principal client_id');
  return {
    subject: expectString(object.subject, 'identity principal subject'),
    issuer: expectString(object.issuer, 'identity principal issuer'),
    display_name: displayName ?? null,
    client_id: clientId ?? null,
    principal_type: expectEnum(
      object.principal_type,
      'identity principal principal_type',
      IDENTITY_PRINCIPAL_TYPES,
    ),
  };
}

function toIdentityResourceCounts(value: unknown): IdentityResourceCounts {
  const object = expectRecord(value, 'identity resource counts');
  return {
    projects: expectNumber(object.projects, 'identity resource counts projects'),
    service_principals: expectNumber(object.service_principals, 'identity resource counts service_principals'),
    identity_mappings: expectNumber(object.identity_mappings, 'identity resource counts identity_mappings'),
    sessions: expectNumber(object.sessions, 'identity resource counts sessions'),
    active_sessions: expectNumber(object.active_sessions, 'identity resource counts active_sessions'),
    session_templates: expectNumber(object.session_templates, 'identity resource counts session_templates'),
    browser_contexts: expectNumber(object.browser_contexts, 'identity resource counts browser_contexts'),
    egress_profiles: expectNumber(object.egress_profiles, 'identity resource counts egress_profiles'),
    credential_bindings: expectNumber(object.credential_bindings, 'identity resource counts credential_bindings'),
    file_workspaces: expectNumber(object.file_workspaces, 'identity resource counts file_workspaces'),
    workflow_definitions: expectNumber(object.workflow_definitions, 'identity resource counts workflow_definitions'),
    workflow_runs: expectNumber(object.workflow_runs, 'identity resource counts workflow_runs'),
    active_workflow_runs: expectNumber(object.active_workflow_runs, 'identity resource counts active_workflow_runs'),
    automation_tasks: expectNumber(object.automation_tasks, 'identity resource counts automation_tasks'),
    active_automation_tasks: expectNumber(object.active_automation_tasks, 'identity resource counts active_automation_tasks'),
    extension_definitions: expectNumber(object.extension_definitions, 'identity resource counts extension_definitions'),
    delegated_principals: expectNumber(object.delegated_principals, 'identity resource counts delegated_principals'),
  };
}

function toIdentityMappingReview(value: unknown): IdentityMappingReviewResource {
  const object = expectRecord(value, 'identity mapping review');
  return {
    ...toIdentityMappingResource(object),
    effective_for_principal: expectBoolean(
      object.effective_for_principal,
      'identity mapping review effective_for_principal',
    ),
  };
}

function toIdentityUnmappedPrincipalSignal(value: unknown): IdentityUnmappedPrincipalSignalResource {
  const object = expectRecord(value, 'identity unmapped principal signal');
  const displayName = optionalString(object.display_name, 'identity unmapped principal signal display_name');
  return {
    kind: expectEnum(object.kind, 'identity unmapped principal signal kind', IDENTITY_MAPPING_KINDS),
    issuer: expectString(object.issuer, 'identity unmapped principal signal issuer'),
    external_id: expectString(object.external_id, 'identity unmapped principal signal external_id'),
    display_name: displayName ?? null,
    reason: expectString(object.reason, 'identity unmapped principal signal reason'),
  };
}

function toIdentityDelegatedPrincipal(value: unknown): IdentityDelegatedPrincipalResource {
  const object = expectRecord(value, 'identity delegated principal');
  const displayName = optionalString(object.display_name, 'identity delegated principal display_name');
  const registeredServicePrincipalId = optionalString(
    object.registered_service_principal_id,
    'identity delegated principal registered_service_principal_id',
  );
  return {
    client_id: expectString(object.client_id, 'identity delegated principal client_id'),
    issuer: expectString(object.issuer, 'identity delegated principal issuer'),
    display_name: displayName ?? null,
    registered: expectBoolean(object.registered, 'identity delegated principal registered'),
    registered_service_principal_id: registeredServicePrincipalId ?? null,
    state: optionalServicePrincipalState(object.state, 'identity delegated principal state') ?? null,
    session_count: expectNumber(object.session_count, 'identity delegated principal session_count'),
    active_session_count: expectNumber(
      object.active_session_count,
      'identity delegated principal active_session_count',
    ),
    session_ids: toStringArray(object.session_ids ?? [], 'identity delegated principal session_ids'),
  };
}

function toIdentityServicePrincipalReview(value: unknown): IdentityServicePrincipalReviewResource {
  const object = expectRecord(value, 'identity service principal review');
  return {
    ...toServicePrincipalResource(object),
    delegated_session_count: expectNumber(
      object.delegated_session_count,
      'identity service principal review delegated_session_count',
    ),
    active_delegated_session_count: expectNumber(
      object.active_delegated_session_count,
      'identity service principal review active_delegated_session_count',
    ),
    delegated_session_ids: toStringArray(
      object.delegated_session_ids ?? [],
      'identity service principal review delegated_session_ids',
    ),
  };
}

function toIdentityMappingResource(value: unknown): IdentityMappingResource {
  const object = expectRecord(value, 'identity mapping resource');
  const description = optionalString(object.description, 'identity mapping description');
  const claimName = optionalString(object.claim_name, 'identity mapping claim_name');
  const servicePrincipalId = optionalString(object.service_principal_id, 'identity mapping service_principal_id');
  const lastSeenAt = optionalString(object.last_seen_at, 'identity mapping last_seen_at');
  return {
    id: expectString(object.id, 'identity mapping id'),
    name: expectString(object.name, 'identity mapping name'),
    description: description ?? null,
    kind: expectEnum(object.kind, 'identity mapping kind', IDENTITY_MAPPING_KINDS),
    issuer: expectString(object.issuer, 'identity mapping issuer'),
    external_id: expectString(object.external_id, 'identity mapping external_id'),
    claim_name: claimName ?? null,
    service_principal_id: servicePrincipalId ?? null,
    project_id: expectString(object.project_id, 'identity mapping project_id'),
    labels: expectStringRecord(object.labels ?? {}, 'identity mapping labels'),
    scopes: toStringArray(object.scopes ?? [], 'identity mapping scopes'),
    state: expectEnum(object.state, 'identity mapping state', IDENTITY_MAPPING_STATES),
    last_seen_at: lastSeenAt ?? null,
    created_at: expectString(object.created_at, 'identity mapping created_at'),
    updated_at: expectString(object.updated_at, 'identity mapping updated_at'),
  };
}

function toServicePrincipalResource(value: unknown): ServicePrincipalResource {
  const object = expectRecord(value, 'service principal resource');
  const description = optionalString(object.description, 'service principal description');
  const lastSeenAt = optionalString(object.last_seen_at, 'service principal last_seen_at');
  const lastDelegatedAt = optionalString(object.last_delegated_at, 'service principal last_delegated_at');
  return {
    id: expectString(object.id, 'service principal id'),
    name: expectString(object.name, 'service principal name'),
    description: description ?? null,
    client_id: expectString(object.client_id, 'service principal client_id'),
    issuer: expectString(object.issuer, 'service principal issuer'),
    labels: expectStringRecord(object.labels ?? {}, 'service principal labels'),
    scopes: toStringArray(object.scopes ?? [], 'service principal scopes'),
    allowed_project_ids: toStringArray(object.allowed_project_ids ?? [], 'service principal allowed_project_ids'),
    state: expectEnum(object.state, 'service principal state', SERVICE_PRINCIPAL_STATES),
    last_seen_at: lastSeenAt ?? null,
    last_delegated_at: lastDelegatedAt ?? null,
    created_at: expectString(object.created_at, 'service principal created_at'),
    updated_at: expectString(object.updated_at, 'service principal updated_at'),
  };
}

function optionalServicePrincipalState(
  value: unknown,
  label: string,
): ServicePrincipalState | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return expectEnum(value, label, SERVICE_PRINCIPAL_STATES);
}

function toProjectQuotas(value: unknown): ProjectQuotas {
  const object = value === undefined || value === null
    ? {}
    : expectRecord(value, 'project quotas');
  return {
    max_active_sessions: optionalNumber(
      object.max_active_sessions,
      'project quotas max_active_sessions',
    ) ?? null,
    max_active_workflow_runs: optionalNumber(
      object.max_active_workflow_runs,
      'project quotas max_active_workflow_runs',
    ) ?? null,
    max_retained_storage_bytes: optionalNumber(
      object.max_retained_storage_bytes,
      'project quotas max_retained_storage_bytes',
    ) ?? null,
  };
}

function toProjectUsage(value: unknown): ProjectUsageResource {
  const object = expectRecord(value, 'project usage');
  return {
    project_id: expectString(object.project_id, 'project usage project_id'),
    active_sessions: expectNumber(object.active_sessions, 'project usage active_sessions'),
    max_active_sessions: optionalNumber(
      object.max_active_sessions,
      'project usage max_active_sessions',
    ) ?? null,
    active_workflow_runs: expectNumber(
      object.active_workflow_runs,
      'project usage active_workflow_runs',
    ),
    max_active_workflow_runs: optionalNumber(
      object.max_active_workflow_runs,
      'project usage max_active_workflow_runs',
    ) ?? null,
    retained_storage_bytes: expectNumber(
      object.retained_storage_bytes,
      'project usage retained_storage_bytes',
    ),
    max_retained_storage_bytes: optionalNumber(
      object.max_retained_storage_bytes,
      'project usage max_retained_storage_bytes',
    ) ?? null,
    observed_at: expectString(object.observed_at, 'project usage observed_at'),
  };
}

function toSessionProjectResource(value: unknown): SessionProjectResource | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  const object = expectRecord(value, 'session project');
  return {
    id: expectString(object.id, 'session project id'),
    name: expectString(object.name, 'session project name'),
    state: expectEnum(object.state, 'session project state', PROJECT_STATES),
  };
}

function toProjectAdmissionDecision(value: unknown): ProjectAdmissionDecision | null {
  if (value === undefined || value === null) {
    return null;
  }
  const object = expectRecord(value, 'project admission decision');
  return {
    state: expectEnum(object.state, 'project admission state', PROJECT_ADMISSION_STATES),
    reason_code: expectEnum(
      object.reason_code,
      'project admission reason_code',
      PROJECT_ADMISSION_REASON_CODES,
    ),
    message: expectString(object.message, 'project admission message'),
    project_id: optionalString(object.project_id, 'project admission project_id') ?? null,
    active_sessions: optionalNumber(
      object.active_sessions,
      'project admission active_sessions',
    ) ?? null,
    max_active_sessions: optionalNumber(
      object.max_active_sessions,
      'project admission max_active_sessions',
    ) ?? null,
    checked_at: expectString(object.checked_at, 'project admission checked_at'),
  };
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
  const credentialBindingId = optionalString(
    object.credential_binding_id,
    'egress profile proxy credential_binding_id',
  );
  return {
    url: expectString(object.url, 'egress profile proxy url'),
    credential_binding_id: credentialBindingId ?? null,
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
    proxy_auth_configured: expectBoolean(
      object.proxy_auth_configured ?? false,
      'egress profile effective proxy_auth_configured',
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
    proxy_auth_configured: expectBoolean(
      object.proxy_auth_configured ?? false,
      'egress diagnostics proxy_auth_configured',
    ),
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
    proxy_auth_configured: expectBoolean(
      object.proxy_auth_configured ?? false,
      'session effective_egress proxy_auth_configured',
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
  const projectId = optionalString(object.project_id, 'session template defaults project_id');
  return {
    project_id: projectId ?? null,
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
