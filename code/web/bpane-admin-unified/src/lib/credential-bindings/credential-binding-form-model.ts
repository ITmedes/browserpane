import { parseKeyValueLabels, splitFormEntries } from '$lib/application/admin-form-utils';
import type {
  CreateCredentialBindingRequest,
  CredentialInjectionMode,
} from './credential-binding-types';

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
  {
    value: 'form_fill',
    label: 'Form fill',
    description: 'Fill configured page selectors with stored values.',
  },
  {
    value: 'cookie_seed',
    label: 'Cookie seed',
    description: 'Seed browser-context cookies before the workflow step.',
  },
  {
    value: 'storage_seed',
    label: 'Storage seed',
    description: 'Seed local and session storage for an allowed origin.',
  },
  {
    value: 'totp_fill',
    label: 'TOTP fill',
    description: 'Generate and fill a time-based one-time password.',
  },
] satisfies readonly {
  readonly value: CredentialInjectionMode;
  readonly label: string;
  readonly description: string;
}[];

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

export function validateCredentialBindingDraft(
  draft: CredentialBindingDraft,
): CredentialBindingValidation {
  const fieldErrors: Record<string, string[]> = {};
  const name = draft.name.trim();
  if (!name) fieldErrors.name = ['Name is required.'];
  if (draft.scope === 'project' && !draft.projectId) fieldErrors.projectId = ['Select a project.'];

  const allowedOrigins = splitFormEntries(draft.allowedOriginsText);
  const invalidOrigin = allowedOrigins.find((origin) => !isHttpOrigin(origin));
  if (invalidOrigin) {
    fieldErrors.allowedOrigins = [`${invalidOrigin} must be an HTTP or HTTPS origin.`];
  }

  const labels = parseKeyValueLabels(draft.labelsText);
  if (!labels.ok) fieldErrors.labels = [labels.error];
  const secret = validateSecret(draft, fieldErrors);
  const totp = validateTotp(draft, fieldErrors);

  const valid = Object.keys(fieldErrors).length === 0;
  if (!valid || !labels.ok) return { valid: false, request: null, fieldErrors };
  return {
    valid: true,
    fieldErrors,
    request: {
      project_id: draft.scope === 'project' ? draft.projectId : null,
      name,
      provider: 'vault_kv_v2',
      ...secret,
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

function validateSecret(
  draft: CredentialBindingDraft,
  fieldErrors: Record<string, string[]>,
): Pick<CreateCredentialBindingRequest, 'external_ref' | 'secret_payload'> {
  const externalRef = draft.externalRef.trim();
  if (draft.secretSource === 'external_ref') {
    if (!externalRef) fieldErrors.externalRef = ['Existing provider reference is required.'];
    return { external_ref: externalRef };
  }
  try {
    const secretPayload: unknown = JSON.parse(draft.secretPayloadText);
    if (!secretPayload || typeof secretPayload !== 'object' || Array.isArray(secretPayload)) {
      fieldErrors.secretPayload = ['Secret payload must be a JSON object.'];
    }
    return { secret_payload: secretPayload };
  } catch {
    fieldErrors.secretPayload = ['Secret payload must be valid JSON.'];
    return { secret_payload: undefined };
  }
}

function validateTotp(draft: CredentialBindingDraft, fieldErrors: Record<string, string[]>) {
  if (draft.injectionMode !== 'totp_fill') return null;
  const period = positiveInteger(draft.totpPeriodSec);
  const digits = positiveInteger(draft.totpDigits);
  if (period === null) fieldErrors.totpPeriodSec = ['TOTP period must be a positive integer.'];
  if (digits === null) fieldErrors.totpDigits = ['TOTP digits must be a positive integer.'];
  return period !== null && digits !== null
    ? {
        issuer: draft.totpIssuer.trim() || null,
        account_name: draft.totpAccountName.trim() || null,
        period_sec: period,
        digits,
      }
    : null;
}

function isHttpOrigin(value: string): boolean {
  try {
    const url = new URL(value);
    return (
      (url.protocol === 'http:' || url.protocol === 'https:') &&
      url.origin === value.replace(/\/$/, '')
    );
  } catch {
    return false;
  }
}

function positiveInteger(value: string): number | null {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : null;
}
