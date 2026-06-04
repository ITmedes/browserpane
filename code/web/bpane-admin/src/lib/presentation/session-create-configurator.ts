import type {
  BrowserContextResource,
  CreateBrowserContextCommand,
  CreateSessionCommand,
  EgressProfileResource,
  ProjectResource,
  SessionBrowserContextCommand,
  SessionBrowserContextMode,
  SessionNetworkIdentity,
  SessionTemplateResource,
} from '../api/control-types';
import {
  LOCAL_EGRESS_PRESET_LABEL_KEY,
  LOCAL_EGRESS_PROXY_NAME,
  LOCAL_EGRESS_PROXY_PRESET,
  LOCAL_EGRESS_TLS_NAME,
  LOCAL_EGRESS_TLS_PRESET,
} from '../api/local-egress-preset-types';

export const SESSION_CREATE_OWNER_MODES = [
  {
    value: 'collaborative',
    label: 'Collaborative',
  },
  {
    value: 'exclusive_browser_owner',
    label: 'Exclusive browser owner',
  },
] as const;

export const DEFAULT_SESSION_CREATE_OWNER_MODE = 'collaborative';

export const SESSION_BROWSER_CONTEXT_MODES = [
  {
    value: 'fresh',
    label: 'Fresh profile',
  },
  {
    value: 'ephemeral',
    label: 'Ephemeral profile',
  },
  {
    value: 'reusable',
    label: 'Reusable context',
  },
] as const;

const OWNER_MODE_VALUES = new Set<string>(
  SESSION_CREATE_OWNER_MODES.map((mode) => mode.value),
);
const BROWSER_CONTEXT_MODE_VALUES = new Set<string>(
  SESSION_BROWSER_CONTEXT_MODES.map((mode) => mode.value),
);

export type SessionCreateFormState = {
  readonly projectId?: string;
  readonly templateId: string;
  readonly ownerMode: string;
  readonly idleTimeoutSec: string;
  readonly labels: string;
  readonly locale?: string;
  readonly languages?: string;
  readonly timezone?: string;
  readonly geolocationLatitude?: string;
  readonly geolocationLongitude?: string;
  readonly geolocationAccuracyMeters?: string;
  readonly userAgent?: string;
  readonly browserIdentity?: string;
  readonly egressProfileId?: string;
  readonly browserContextMode?: string;
  readonly browserContextId?: string;
  readonly browserContexts?: readonly BrowserContextResource[];
  readonly egressProfiles?: readonly EgressProfileResource[];
  readonly projects?: readonly ProjectResource[];
};

export type SessionCreateValidation = {
  readonly command: CreateSessionCommand | null;
  readonly errors: readonly string[];
  readonly preview: string;
};

type MutableCreateSessionCommand = {
  project_id?: string;
  template_id?: string;
  browser_context?: SessionBrowserContextCommand;
  network_identity?: SessionNetworkIdentity;
  owner_mode?: string;
  idle_timeout_sec?: number;
  labels?: Readonly<Record<string, string>>;
};

export type BrowserContextCreateFormState = {
  readonly projectId?: string;
  readonly projects?: readonly ProjectResource[];
  readonly name: string;
  readonly labels: string;
  readonly retentionDays?: string;
  readonly maxProfileStorageMb?: string;
};

export type BrowserContextCreateValidation = {
  readonly command: CreateBrowserContextCommand | null;
  readonly errors: readonly string[];
};

export function defaultSessionCreateFormState(): SessionCreateFormState {
  return {
    projectId: '',
    templateId: '',
    ownerMode: DEFAULT_SESSION_CREATE_OWNER_MODE,
    idleTimeoutSec: '',
    labels: '',
    locale: '',
    languages: '',
    timezone: '',
    geolocationLatitude: '',
    geolocationLongitude: '',
    geolocationAccuracyMeters: '',
    userAgent: '',
    browserIdentity: '',
    egressProfileId: '',
    browserContextMode: 'fresh',
    browserContextId: '',
  };
}

export function validateSessionCreateForm(
  state: SessionCreateFormState,
): SessionCreateValidation {
  const errors: string[] = [];
  const command: MutableCreateSessionCommand = {};
  const projectId = (state.projectId ?? '').trim();
  if (projectId) {
    const project = state.projects?.find((entry) => entry.id === projectId) ?? null;
    if (state.projects && state.projects.length > 0 && !project) {
      errors.push('Selected project is not available.');
    } else if (project?.state === 'archived') {
      errors.push('Selected project is archived.');
    } else {
      command.project_id = projectId;
    }
  }
  const templateId = state.templateId.trim();
  if (templateId) {
    command.template_id = templateId;
  }

  const browserContextMode = (state.browserContextMode ?? 'fresh').trim();
  const browserContextId = (state.browserContextId ?? '').trim();
  if (!BROWSER_CONTEXT_MODE_VALUES.has(browserContextMode)) {
    errors.push(`Browser context mode "${browserContextMode}" is not supported.`);
  } else if (browserContextMode === 'reusable') {
    if (!browserContextId) {
      errors.push('Reusable browser context requires a selected context.');
    } else {
      const browserContext = state.browserContexts?.find((context) => context.id === browserContextId) ?? null;
      if (state.browserContexts && state.browserContexts.length > 0 && !browserContext) {
        errors.push('Selected reusable browser context is not available.');
      } else if (browserContext && browserContext.state !== 'ready') {
        errors.push('Selected reusable browser context must be ready.');
      } else if (browserContext && browserContext.persistence_mode !== 'reusable') {
        errors.push('Selected browser context is not reusable.');
      } else if (
        browserContext?.project_id
        && browserContext.project_id !== projectId
      ) {
        errors.push('Selected reusable browser context belongs to a different project.');
      } else {
        command.browser_context = {
          mode: 'reusable',
          context_id: browserContextId,
        };
      }
    }
  } else if (browserContextId) {
    errors.push('Browser context id can only be set for reusable mode.');
  } else if (browserContextMode === 'ephemeral') {
    command.browser_context = { mode: 'ephemeral' };
  }

  const ownerMode = state.ownerMode.trim();
  if (!ownerMode) {
    // Empty means "let the selected template or backend default decide".
  } else if (OWNER_MODE_VALUES.has(ownerMode)) {
    command.owner_mode = ownerMode;
  } else {
    errors.push(`Owner mode "${ownerMode}" is not supported.`);
  }

  const idleTimeoutSec = parseIdleTimeoutSec(state.idleTimeoutSec, errors);
  if (idleTimeoutSec !== undefined) {
    command.idle_timeout_sec = idleTimeoutSec;
  }

  const labels = parseSessionCreateLabels(state.labels, errors);
  if (Object.keys(labels).length > 0) {
    command.labels = labels;
  }
  const networkIdentity = parseNetworkIdentity(state, errors);
  if (networkIdentity) {
    command.network_identity = networkIdentity;
  }

  return {
    command: errors.length === 0 ? command : null,
    errors,
    preview: errors.length === 0
      ? JSON.stringify(command, null, 2)
      : 'Fix validation errors to preview the API payload.',
  };
}

function parseNetworkIdentity(
  state: SessionCreateFormState,
  errors: string[],
): SessionNetworkIdentity | undefined {
  const identity: {
    locale?: string;
    languages?: string[];
    timezone?: string;
    geolocation?: { latitude: number; longitude: number; accuracy_meters?: number };
    user_agent?: string;
    browser_identity?: string;
    egress_profile_id?: string;
  } = {};
  const locale = (state.locale ?? '').trim();
  if (locale) {
    identity.locale = locale;
  }
  const languages = parseList(state.languages ?? '', 'Language', errors);
  if (languages.length > 0) {
    identity.languages = languages;
  }
  const timezone = (state.timezone ?? '').trim();
  if (timezone) {
    identity.timezone = timezone;
  }
  const userAgent = (state.userAgent ?? '').trim();
  if (userAgent) {
    if (/[\r\n]/u.test(userAgent)) {
      errors.push('User agent must be a single line.');
    } else {
      identity.user_agent = userAgent;
    }
  }
  const browserIdentity = (state.browserIdentity ?? '').trim();
  if (browserIdentity) {
    identity.browser_identity = browserIdentity;
  }
  const egressProfileId = (state.egressProfileId ?? '').trim();
  if (egressProfileId) {
    const profile = state.egressProfiles?.find((entry) => entry.id === egressProfileId) ?? null;
    if (state.egressProfiles && state.egressProfiles.length > 0 && !profile) {
      errors.push('Selected egress profile is not available.');
    } else if (profile?.state === 'disabled') {
      errors.push('Selected egress profile is disabled.');
    } else {
      identity.egress_profile_id = egressProfileId;
    }
  }

  const latitudeText = (state.geolocationLatitude ?? '').trim();
  const longitudeText = (state.geolocationLongitude ?? '').trim();
  const accuracyText = (state.geolocationAccuracyMeters ?? '').trim();
  if (latitudeText || longitudeText || accuracyText) {
    if (!latitudeText || !longitudeText) {
      errors.push('Geolocation requires latitude and longitude together.');
    } else {
      const latitude = parseFiniteNumber(latitudeText, 'Latitude', errors);
      const longitude = parseFiniteNumber(longitudeText, 'Longitude', errors);
      const accuracy = accuracyText
        ? parseFiniteNumber(accuracyText, 'Geolocation accuracy', errors)
        : undefined;
      if (latitude !== undefined && (latitude < -90 || latitude > 90)) {
        errors.push('Latitude must be between -90 and 90.');
      }
      if (longitude !== undefined && (longitude < -180 || longitude > 180)) {
        errors.push('Longitude must be between -180 and 180.');
      }
      if (accuracy !== undefined && accuracy <= 0) {
        errors.push('Geolocation accuracy must be greater than zero.');
      }
      if (
        latitude !== undefined
        && longitude !== undefined
        && latitude >= -90
        && latitude <= 90
        && longitude >= -180
        && longitude <= 180
        && (accuracy === undefined || accuracy > 0)
      ) {
        identity.geolocation = {
          latitude,
          longitude,
          ...(accuracy !== undefined ? { accuracy_meters: accuracy } : {}),
        };
      }
    }
  }

  return Object.keys(identity).length > 0 ? identity : undefined;
}

function parseList(value: string, label: string, errors: string[]): string[] {
  const entries: string[] = [];
  const seen = new Set<string>();
  for (const rawPart of value.split(/[\n,]/u)) {
    const part = rawPart.trim();
    if (!part) {
      continue;
    }
    if (seen.has(part)) {
      errors.push(`${label} "${part}" is duplicated.`);
      continue;
    }
    seen.add(part);
    entries.push(part);
  }
  return entries;
}

function parseFiniteNumber(
  value: string,
  label: string,
  errors: string[],
): number | undefined {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    errors.push(`${label} must be a finite number.`);
    return undefined;
  }
  return parsed;
}

export function validateBrowserContextCreateForm(
  state: BrowserContextCreateFormState,
): BrowserContextCreateValidation {
  const errors: string[] = [];
  const name = state.name.trim();
  if (!name) {
    errors.push('Browser context name is required.');
  }
  const projectId = (state.projectId ?? '').trim();
  if (projectId) {
    const project = state.projects?.find((entry) => entry.id === projectId) ?? null;
    if (state.projects && state.projects.length > 0 && !project) {
      errors.push('Selected project is not available.');
    } else if (project?.state === 'archived') {
      errors.push('Selected project is archived.');
    }
  }
  const labels = parseSessionCreateLabels(state.labels, errors);
  const retentionSec = parseRetentionDays(state.retentionDays ?? '', errors);
  const maxProfileStorageBytes = parseMaxProfileStorageMb(state.maxProfileStorageMb ?? '', errors);
  return {
    command: errors.length === 0
      ? {
          name,
          ...(projectId ? { project_id: projectId } : {}),
          labels,
          persistence_mode: 'reusable',
          ...(retentionSec !== undefined ? { retention_sec: retentionSec } : {}),
          ...(maxProfileStorageBytes !== undefined ? { max_profile_storage_bytes: maxProfileStorageBytes } : {}),
        }
      : null,
    errors,
  };
}

export function parseSessionCreateLabels(
  value: string,
  errors: string[] = [],
): Readonly<Record<string, string>> {
  const labels: Record<string, string> = {};
  const seen = new Set<string>();
  for (const rawPart of value.split(/[\n,]/u)) {
    const part = rawPart.trim();
    if (!part) {
      continue;
    }
    const separator = part.indexOf('=');
    if (separator <= 0) {
      errors.push(`Label "${part}" must use key=value.`);
      continue;
    }
    const key = part.slice(0, separator).trim();
    const labelValue = part.slice(separator + 1).trim();
    if (!key || !labelValue) {
      errors.push(`Label "${part}" must use non-empty key and value.`);
      continue;
    }
    if (seen.has(key)) {
      errors.push(`Label "${key}" is duplicated.`);
      continue;
    }
    seen.add(key);
    labels[key] = labelValue;
  }
  return labels;
}

function parseIdleTimeoutSec(value: string, errors: string[]): number | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  const parsed = Number(trimmed);
  if (!Number.isInteger(parsed) || parsed < 1) {
    errors.push('Idle timeout must be a positive whole number of seconds.');
    return undefined;
  }
  if (!Number.isSafeInteger(parsed)) {
    errors.push('Idle timeout is too large to send safely.');
    return undefined;
  }
  return parsed;
}

export function sessionTemplateDefaultsSummary(
  template: SessionTemplateResource | null | undefined,
): string {
  if (!template) {
    return 'No template defaults selected.';
  }
  const facts: string[] = [];
  const defaults = template.defaults;
  if (defaults.owner_mode) {
    facts.push(`owner=${defaults.owner_mode}`);
  }
  if (defaults.project_id) {
    facts.push(`project=${shortId(defaults.project_id)}`);
  }
  if (defaults.idle_timeout_sec) {
    facts.push(`idle=${defaults.idle_timeout_sec}s`);
  }
  if (defaults.viewport) {
    facts.push(`viewport=${defaults.viewport.width}x${defaults.viewport.height}`);
  }
  const labels = Object.entries(defaults.labels ?? {});
  if (labels.length > 0) {
    facts.push(`labels=${labels.map(([key, value]) => `${key}=${value}`).join(',')}`);
  }
  const contextKeys = Object.keys(defaults.integration_context ?? {}).sort();
  if (contextKeys.length > 0) {
    facts.push(`integration=${contextKeys.join(',')}`);
  }
  if (defaults.recording) {
    const mode = typeof defaults.recording.mode === 'string' ? defaults.recording.mode : 'configured';
    facts.push(`recording=${mode}`);
  }
  const networkSummary = networkIdentitySummary(defaults.network_identity ?? null);
  if (networkSummary !== 'default network identity') {
    facts.push(networkSummary);
  }
  return facts.length > 0 ? facts.join(' | ') : 'Template has no create-session defaults.';
}

export function networkIdentitySummary(
  identity: SessionNetworkIdentity | null | undefined,
  egressProfiles: readonly EgressProfileResource[] = [],
): string {
  if (!identity) {
    return 'default network identity';
  }
  const facts: string[] = [];
  if (identity.locale) {
    facts.push(`locale=${identity.locale}`);
  }
  if (identity.languages && identity.languages.length > 0) {
    facts.push(`languages=${identity.languages.join(',')}`);
  }
  if (identity.timezone) {
    facts.push(`timezone=${identity.timezone}`);
  }
  if (identity.geolocation) {
    facts.push(`geo=${identity.geolocation.latitude},${identity.geolocation.longitude}`);
  }
  if (identity.browser_identity) {
    facts.push(`browser=${identity.browser_identity}`);
  } else if (identity.user_agent) {
    facts.push('user-agent=custom');
  }
  if (identity.egress_profile_id) {
    const profile = egressProfiles.find((entry) => entry.id === identity.egress_profile_id);
    facts.push(`egress=${profile ? profile.name : shortId(identity.egress_profile_id)}`);
  }
  return facts.length > 0 ? facts.join(' | ') : 'default network identity';
}

export function egressProfileOptionLabel(profile: EgressProfileResource): string {
  const signals = [
    profile.state,
    profile.effective.proxy_configured ? 'proxy' : null,
    profile.effective.proxy_auth_configured ? 'proxy auth' : null,
    profile.effective.tls_interception_enabled ? 'TLS inspect' : null,
    profile.effective.sensitive_log_sink_configured ? 'log sink' : null,
    profile.effective.custom_ca_configured ? 'custom CA' : null,
    profile.effective.bypass_rule_count > 0 ? `${profile.effective.bypass_rule_count} bypass` : null,
  ].filter(Boolean);
  return `${profile.name} (${signals.join(', ')})`;
}

export function projectOptionLabel(project: ProjectResource): string {
  const signals = [
    project.state,
    `sessions=${usageFraction(project.usage.active_sessions, project.usage.max_active_sessions)}`,
    project.usage.queued_sessions > 0 ? `queued=${project.usage.queued_sessions}` : null,
    project.usage.session_creations > 0 ? `created=${project.usage.session_creations}` : null,
    project.usage.max_active_workflow_runs !== null && project.usage.max_active_workflow_runs !== undefined
      ? `workflows=${usageFraction(project.usage.active_workflow_runs, project.usage.max_active_workflow_runs)}`
      : null,
    project.usage.runtime_usage_ms > 0 ? `runtime=${formatDurationMs(project.usage.runtime_usage_ms)}` : null,
    project.usage.egress_total_bytes > 0 ? `egress_bytes=${formatBytes(project.usage.egress_total_bytes)}` : null,
    project.policy.allowed_session_template_ids.length > 0
      ? `templates=${project.policy.allowed_session_template_ids.length}`
      : null,
    project.policy.allowed_egress_profile_ids.length > 0
      ? `egress=${project.policy.allowed_egress_profile_ids.length}`
      : null,
  ].filter(Boolean);
  return `${project.name} (${signals.join(', ')})`;
}

export function projectUsageSummary(project: ProjectResource | null | undefined): string {
  if (!project) {
    return 'No project selected; the session remains owner-scoped.';
  }
  const facts = [
    `state=${project.state}`,
    `sessions=${usageFraction(project.usage.active_sessions, project.usage.max_active_sessions)}`,
    `queued_sessions=${project.usage.queued_sessions}`,
    `created_sessions=${project.usage.session_creations}`,
    `workflow_runs=${usageFraction(project.usage.active_workflow_runs, project.usage.max_active_workflow_runs)}`,
    `runtime=${formatDurationMs(project.usage.runtime_usage_ms)}`,
    `egress_bytes=${formatBytes(project.usage.egress_total_bytes)}`,
    `storage=${usageFraction(project.usage.retained_storage_bytes, project.usage.max_retained_storage_bytes)}`,
    `policy=${projectPolicySummary(project)}`,
  ];
  const labels = Object.entries(project.labels).sort(([left], [right]) => left.localeCompare(right));
  if (labels.length > 0) {
    facts.push(`labels=${labels.map(([key, value]) => `${key}=${value}`).join(',')}`);
  }
  return facts.join(' | ');
}

function projectPolicySummary(project: ProjectResource): string {
  const facts = [];
  if (project.policy.allowed_session_template_ids.length > 0) {
    facts.push(`${project.policy.allowed_session_template_ids.length} templates`);
  }
  if (project.policy.allowed_egress_profile_ids.length > 0) {
    facts.push(`${project.policy.allowed_egress_profile_ids.length} egress profiles`);
  }
  return facts.length > 0 ? facts.join(',') : 'unrestricted';
}

export function egressProfileKind(profile: EgressProfileResource): 'tls_interceptor' | 'proxy' | 'other' {
  if (profile.effective.tls_interception_enabled || profile.traffic_observation.mode === 'tls_intercept') {
    return 'tls_interceptor';
  }
  if (profile.effective.proxy_configured || profile.proxy) {
    return 'proxy';
  }
  return 'other';
}

export function isLocalProxyEgressPreset(profile: EgressProfileResource): boolean {
  return profile.name === LOCAL_EGRESS_PROXY_NAME
    || profile.labels[LOCAL_EGRESS_PRESET_LABEL_KEY] === LOCAL_EGRESS_PROXY_PRESET;
}

export function isLocalTlsInterceptorEgressPreset(profile: EgressProfileResource): boolean {
  return profile.name === LOCAL_EGRESS_TLS_NAME
    || profile.labels[LOCAL_EGRESS_PRESET_LABEL_KEY] === LOCAL_EGRESS_TLS_PRESET;
}

export function browserContextOptionLabel(context: BrowserContextResource): string {
  const stateSuffix = context.state === 'ready' ? '' : `, ${context.state}`;
  const projectSuffix = context.project?.name
    ? `, project=${context.project.name}`
    : context.project_id
      ? `, project=${shortId(context.project_id)}`
      : '';
  return `${context.name} (${shortId(context.id)}${stateSuffix}${projectSuffix})`;
}

export function sessionBrowserContextSummary(
  mode: SessionBrowserContextMode | string,
  context: BrowserContextResource | null | undefined,
): string {
  if (mode === 'fresh') {
    return 'New session starts with a fresh persisted browser profile.';
  }
  if (mode === 'ephemeral') {
    return 'New session starts with a temporary profile that is discarded after use.';
  }
  if (!context) {
    return 'Select a ready reusable context before creating the session.';
  }
  const facts = [
    `state=${context.state}`,
    `persistence=${context.persistence_mode}`,
    context.project?.name
      ? `project=${context.project.name}`
      : context.project_id
        ? `project=${shortId(context.project_id)}`
        : 'owner-scoped',
    context.last_used_at ? `last_used=${context.last_used_at}` : 'never used',
  ];
  const labels = Object.entries(context.labels).sort(([left], [right]) => left.localeCompare(right));
  if (labels.length > 0) {
    facts.push(`labels=${labels.map(([key, value]) => `${key}=${value}`).join(',')}`);
  }
  if (context.retention_sec) {
    facts.push(`retention=${formatDuration(context.retention_sec)}`);
  }
  if (context.max_profile_storage_bytes) {
    facts.push(`storage_limit=${formatBytes(context.max_profile_storage_bytes)}`);
  }
  return facts.join(' | ');
}

function parseRetentionDays(value: string, errors: string[]): number | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  const parsed = Number(trimmed);
  if (!Number.isInteger(parsed) || parsed < 1) {
    errors.push('Retention days must be a positive whole number.');
    return undefined;
  }
  const seconds = parsed * 86400;
  if (!Number.isSafeInteger(seconds)) {
    errors.push('Retention days is too large to send safely.');
    return undefined;
  }
  if (seconds > 4_294_967_295) {
    errors.push('Retention days exceeds the API limit.');
    return undefined;
  }
  return seconds;
}

function parseMaxProfileStorageMb(value: string, errors: string[]): number | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  const parsed = Number(trimmed);
  if (!Number.isInteger(parsed) || parsed < 1) {
    errors.push('Max profile storage must be a positive whole number of MB.');
    return undefined;
  }
  const bytes = parsed * 1024 * 1024;
  if (!Number.isSafeInteger(bytes)) {
    errors.push('Max profile storage is too large to send safely.');
    return undefined;
  }
  return bytes;
}

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) {
    return '0B';
  }
  if (value % (1024 * 1024 * 1024) === 0) {
    const gib = value / (1024 * 1024 * 1024);
    return `${gib}GiB`;
  }
  if (value % (1024 * 1024) === 0) {
    const mib = value / (1024 * 1024);
    return `${mib}MiB`;
  }
  if (value % 1024 === 0) {
    const kib = value / 1024;
    return `${kib}KiB`;
  }
  return `${value}B`;
}

function usageFraction(value: number, max: number | null | undefined): string {
  return max === null || max === undefined ? `${value}/unlimited` : `${value}/${max}`;
}

function formatDuration(seconds: number): string {
  if (seconds % 86400 === 0) {
    const days = seconds / 86400;
    return `${days}d`;
  }
  if (seconds % 3600 === 0) {
    const hours = seconds / 3600;
    return `${hours}h`;
  }
  return `${seconds}s`;
}

function formatDurationMs(milliseconds: number): string {
  if (!Number.isFinite(milliseconds) || milliseconds <= 0) {
    return '0s';
  }
  const seconds = Math.floor(milliseconds / 1000);
  if (seconds < 60) {
    return `${seconds}s`;
  }
  if (seconds % 86400 === 0) {
    return `${seconds / 86400}d`;
  }
  if (seconds % 3600 === 0) {
    return `${seconds / 3600}h`;
  }
  if (seconds % 60 === 0) {
    return `${seconds / 60}m`;
  }
  return `${seconds}s`;
}

function shortId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}
