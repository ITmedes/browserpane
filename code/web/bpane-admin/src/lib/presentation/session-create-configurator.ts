import type {
  BrowserContextResource,
  CreateBrowserContextCommand,
  CreateSessionCommand,
  SessionBrowserContextCommand,
  SessionBrowserContextMode,
  SessionTemplateResource,
} from '../api/control-types';

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
  readonly templateId: string;
  readonly ownerMode: string;
  readonly idleTimeoutSec: string;
  readonly labels: string;
  readonly browserContextMode?: string;
  readonly browserContextId?: string;
  readonly browserContexts?: readonly BrowserContextResource[];
};

export type SessionCreateValidation = {
  readonly command: CreateSessionCommand | null;
  readonly errors: readonly string[];
  readonly preview: string;
};

type MutableCreateSessionCommand = {
  template_id?: string;
  browser_context?: SessionBrowserContextCommand;
  owner_mode?: string;
  idle_timeout_sec?: number;
  labels?: Readonly<Record<string, string>>;
};

export type BrowserContextCreateFormState = {
  readonly name: string;
  readonly labels: string;
  readonly retentionDays?: string;
};

export type BrowserContextCreateValidation = {
  readonly command: CreateBrowserContextCommand | null;
  readonly errors: readonly string[];
};

export function defaultSessionCreateFormState(): SessionCreateFormState {
  return {
    templateId: '',
    ownerMode: DEFAULT_SESSION_CREATE_OWNER_MODE,
    idleTimeoutSec: '',
    labels: '',
    browserContextMode: 'fresh',
    browserContextId: '',
  };
}

export function validateSessionCreateForm(
  state: SessionCreateFormState,
): SessionCreateValidation {
  const errors: string[] = [];
  const command: MutableCreateSessionCommand = {};
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

  return {
    command: errors.length === 0 ? command : null,
    errors,
    preview: errors.length === 0
      ? JSON.stringify(command, null, 2)
      : 'Fix validation errors to preview the API payload.',
  };
}

export function validateBrowserContextCreateForm(
  state: BrowserContextCreateFormState,
): BrowserContextCreateValidation {
  const errors: string[] = [];
  const name = state.name.trim();
  if (!name) {
    errors.push('Browser context name is required.');
  }
  const labels = parseSessionCreateLabels(state.labels, errors);
  const retentionSec = parseRetentionDays(state.retentionDays ?? '', errors);
  return {
    command: errors.length === 0
      ? {
          name,
          labels,
          persistence_mode: 'reusable',
          ...(retentionSec !== undefined ? { retention_sec: retentionSec } : {}),
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
  return facts.length > 0 ? facts.join(' | ') : 'Template has no create-session defaults.';
}

export function browserContextOptionLabel(context: BrowserContextResource): string {
  const stateSuffix = context.state === 'ready' ? '' : `, ${context.state}`;
  return `${context.name} (${shortId(context.id)}${stateSuffix})`;
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
    context.last_used_at ? `last_used=${context.last_used_at}` : 'never used',
  ];
  const labels = Object.entries(context.labels).sort(([left], [right]) => left.localeCompare(right));
  if (labels.length > 0) {
    facts.push(`labels=${labels.map(([key, value]) => `${key}=${value}`).join(',')}`);
  }
  if (context.retention_sec) {
    facts.push(`retention=${formatDuration(context.retention_sec)}`);
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

function shortId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}
