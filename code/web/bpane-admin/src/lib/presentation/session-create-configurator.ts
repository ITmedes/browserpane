import type { CreateSessionCommand, SessionTemplateResource } from '../api/control-types';

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

const OWNER_MODE_VALUES = new Set<string>(
  SESSION_CREATE_OWNER_MODES.map((mode) => mode.value),
);

export type SessionCreateFormState = {
  readonly templateId: string;
  readonly ownerMode: string;
  readonly idleTimeoutSec: string;
  readonly labels: string;
};

export type SessionCreateValidation = {
  readonly command: CreateSessionCommand | null;
  readonly errors: readonly string[];
  readonly preview: string;
};

type MutableCreateSessionCommand = {
  template_id?: string;
  owner_mode?: string;
  idle_timeout_sec?: number;
  labels?: Readonly<Record<string, string>>;
};

export function defaultSessionCreateFormState(): SessionCreateFormState {
  return {
    templateId: '',
    ownerMode: DEFAULT_SESSION_CREATE_OWNER_MODE,
    idleTimeoutSec: '',
    labels: '',
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
