import type {
  BrowserContextListResponse,
  BrowserContextPersistenceMode,
  BrowserContextResource,
  BrowserContextState,
  BrowserContextUsageResource,
  SessionAutomationDelegate,
  SessionBrowserContextMode,
  SessionBrowserContextResource,
  SessionConnectionCounts,
  SessionConnectInfo,
  SessionAccessTokenResponse,
  SessionListResponse,
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
