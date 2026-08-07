import { parseKeyValueLabels, splitFormEntries } from '$lib/application/admin-form-utils';
import { formatDateTime, type ProjectTone } from '$lib/projects/project-formatters';
import type {
  CreateCredentialBindingRequest,
  CredentialBindingProjectResource,
  CredentialBindingResource,
  CredentialInjectionMode,
} from './credential-binding-types';

export type CredentialBindingOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly bindings: readonly CredentialBindingResource[] };

export type CredentialBindingDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly bindingId: string }
  | { readonly status: 'error'; readonly bindingId: string; readonly message: string }
  | { readonly status: 'ready'; readonly binding: CredentialBindingResource };

export type CredentialBindingProjectOptionsLoadState =
  | { readonly status: 'idle' | 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly projects: readonly CredentialBindingProjectResource[] };

export type CredentialBindingOverviewRow = {
  readonly id: string;
  readonly name: string;
  readonly scope: string;
  readonly scopeTone: ProjectTone;
  readonly provider: string;
  readonly injectionMode: string;
  readonly origins: string;
  readonly namespace: string;
  readonly labels: string;
  readonly updatedAt: string;
};

export type CredentialBindingDraft = {
  scope: 'owner' | 'project';
  projectId: string;
  name: string;
  namespace: string;
  allowedOriginsText: string;
  injectionMode: CredentialInjectionMode;
  secretSource: 'payload' | 'external_ref';
  secretPayloadText: string;
  externalRef: string;
  totpIssuer: string;
  totpAccountName: string;
  totpPeriodSec: string;
  totpDigits: string;
  labelsText: string;
};

export type CredentialBindingValidation = {
  readonly valid: boolean;
  readonly request: CreateCredentialBindingRequest | null;
  readonly fieldErrors: Readonly<Record<string, readonly string[]>>;
};

export const CREDENTIAL_INJECTION_MODE_OPTIONS = [
  { value: 'form_fill', label: 'Form fill', description: 'Fill configured page selectors with stored values.' },
  { value: 'cookie_seed', label: 'Cookie seed', description: 'Seed browser-context cookies before the workflow step.' },
  { value: 'storage_seed', label: 'Storage seed', description: 'Seed local and session storage for an allowed origin.' },
  { value: 'totp_fill', label: 'TOTP fill', description: 'Generate and fill a time-based one-time password.' },
] satisfies readonly { readonly value: CredentialInjectionMode; readonly label: string; readonly description: string }[];

export function buildCredentialBindingOverviewModel(bindings: readonly CredentialBindingResource[]) {
  return {
    metrics: [
      metric('total', 'Bindings', bindings.length),
      metric('owner', 'Owner scoped', bindings.filter((binding) => !binding.project_id).length),
      metric('project', 'Project scoped', bindings.filter((binding) => binding.project_id).length),
      metric('totp', 'TOTP', bindings.filter((binding) => binding.injection_mode === 'totp_fill').length),
    ],
    rows: bindings.map(credentialBindingOverviewRow),
  };
}

export function credentialBindingOverviewRow(binding: CredentialBindingResource): CredentialBindingOverviewRow {
  return {
    id: binding.id,
    name: binding.name,
    scope: binding.project?.name ?? binding.project_id ?? 'Owner scoped',
    scopeTone: binding.project_id ? 'warning' : 'neutral',
    provider: binding.provider === 'vault_kv_v2' ? 'Vault KV v2' : binding.provider,
    injectionMode: injectionModeLabel(binding.injection_mode),
    origins: binding.allowed_origins.length > 0 ? binding.allowed_origins.join(', ') : 'No allowed origins',
    namespace: binding.namespace ?? 'Default namespace',
    labels: labelSummary(binding.labels),
    updatedAt: formatDateTime(binding.updated_at),
  };
}

export function credentialBindingMatchesSearch(row: CredentialBindingOverviewRow, query: string): boolean {
  if (!query) return true;
  return [row.id, row.name, row.scope, row.provider, row.injectionMode, row.origins, row.namespace, row.labels, row.updatedAt]
    .some((value) => value.toLowerCase().includes(query));
}

export function createCredentialBindingDraft(): CredentialBindingDraft {
  return {
    scope: 'owner',
    projectId: '',
    name: '',
    namespace: '',
    allowedOriginsText: '',
    injectionMode: 'form_fill',
    secretSource: 'payload',
    secretPayloadText: '{\n  "fields": [\n    { "selector": "#username", "value": "" }\n  ]\n}',
    externalRef: '',
    totpIssuer: '',
    totpAccountName: '',
    totpPeriodSec: '30',
    totpDigits: '6',
    labelsText: '',
  };
}

export function validateCredentialBindingDraft(draft: CredentialBindingDraft): CredentialBindingValidation {
  const fieldErrors: Record<string, string[]> = {};
  const name = draft.name.trim();
  if (!name) fieldErrors.name = ['Name is required.'];
  if (draft.scope === 'project' && !draft.projectId) fieldErrors.projectId = ['Select a project.'];

  const allowedOrigins = splitFormEntries(draft.allowedOriginsText);
  const invalidOrigin = allowedOrigins.find((origin) => !isHttpOrigin(origin));
  if (invalidOrigin) fieldErrors.allowedOrigins = [`${invalidOrigin} must be an HTTP or HTTPS origin.`];

  const labels = parseKeyValueLabels(draft.labelsText);
  if (!labels.ok) fieldErrors.labels = [labels.error];

  let secretPayload: unknown;
  const externalRef = draft.externalRef.trim();
  if (draft.secretSource === 'external_ref') {
    if (!externalRef) fieldErrors.externalRef = ['Existing provider reference is required.'];
  } else {
    try {
      secretPayload = JSON.parse(draft.secretPayloadText);
      if (!secretPayload || typeof secretPayload !== 'object' || Array.isArray(secretPayload)) {
        fieldErrors.secretPayload = ['Secret payload must be a JSON object.'];
      }
    } catch {
      fieldErrors.secretPayload = ['Secret payload must be valid JSON.'];
    }
  }

  let totp = null;
  if (draft.injectionMode === 'totp_fill') {
    const period = positiveInteger(draft.totpPeriodSec);
    const digits = positiveInteger(draft.totpDigits);
    if (period === null) fieldErrors.totpPeriodSec = ['TOTP period must be a positive integer.'];
    if (digits === null) fieldErrors.totpDigits = ['TOTP digits must be a positive integer.'];
    if (period !== null && digits !== null) {
      totp = {
        issuer: draft.totpIssuer.trim() || null,
        account_name: draft.totpAccountName.trim() || null,
        period_sec: period,
        digits,
      };
    }
  }

  const valid = Object.keys(fieldErrors).length === 0;
  if (!valid || !labels.ok) return { valid: false, request: null, fieldErrors };
  return {
    valid: true,
    fieldErrors,
    request: {
      project_id: draft.scope === 'project' ? draft.projectId : null,
      name,
      provider: 'vault_kv_v2',
      ...(draft.secretSource === 'external_ref' ? { external_ref: externalRef } : { secret_payload: secretPayload }),
      namespace: draft.namespace.trim() || null,
      allowed_origins: allowedOrigins,
      injection_mode: draft.injectionMode,
      totp,
      labels: labels.value,
    },
  };
}

export function injectionModeLabel(mode: CredentialInjectionMode): string {
  return CREDENTIAL_INJECTION_MODE_OPTIONS.find((option) => option.value === mode)?.label ?? mode;
}

export function labelSummary(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels).sort(([left], [right]) => left.localeCompare(right));
  return entries.length === 0 ? 'No labels' : entries.map(([key, value]) => `${key}=${value}`).join(', ');
}

function metric(key: string, label: string, value: number) {
  return { label, value: String(value), testId: `credential-bindings-metric-${key}` };
}

function isHttpOrigin(value: string): boolean {
  try {
    const url = new URL(value);
    return (url.protocol === 'http:' || url.protocol === 'https:') && url.origin === value.replace(/\/$/, '');
  } catch {
    return false;
  }
}

function positiveInteger(value: string): number | null {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : null;
}
