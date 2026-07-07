import { formatDateTime, type ProjectTone } from '$lib/projects/project-formatters';
import type {
  EgressProfileProjectResource,
  EgressProfileResource,
  EgressProfileState,
  EgressTrafficObservationMode,
  UpsertEgressProfileRequest,
} from './egress-profile-types';

export type EgressProfileEditMode = 'create' | 'edit';

export type EgressProfileProjectBinding = 'owner' | 'project';

export type EgressProfileEditValidationField =
  | 'name'
  | 'projectId'
  | 'labels'
  | 'proxyUrl'
  | 'proxyCredentialBindingId'
  | 'bypassRules'
  | 'customCaCertificateRef'
  | 'customCaDisplayName'
  | 'sensitiveLogSinkRef'
  | 'sensitiveLogSinkDisplayName';

export type EgressProfileEditFieldErrors = Partial<Record<EgressProfileEditValidationField, readonly string[]>>;

export type EgressProfileEditDraft = {
  name: string;
  description: string;
  labelsText: string;
  state: EgressProfileState;
  projectBinding: EgressProfileProjectBinding;
  projectId: string;
  proxyEnabled: boolean;
  proxyUrl: string;
  proxyCredentialBindingId: string;
  bypassRulesText: string;
  customCaEnabled: boolean;
  customCaCertificateRef: string;
  customCaDisplayName: string;
  observationMode: EgressTrafficObservationMode;
  sensitiveLogSinkRef: string;
  sensitiveLogSinkDisplayName: string;
};

export type EgressProfileEditValidation = {
  readonly valid: boolean;
  readonly errors: readonly string[];
  readonly fieldErrors: EgressProfileEditFieldErrors;
  readonly request: UpsertEgressProfileRequest | null;
};

export type EgressProfileStatusSummaryItem = {
  readonly label: string;
  readonly value: string;
  readonly tone: ProjectTone;
};

export type EgressProfileStatusSummaryModel = {
  readonly profileId: string;
  readonly stateLabel: string;
  readonly stateTone: ProjectTone;
  readonly healthLabel: string;
  readonly healthTone: ProjectTone;
  readonly proofLabel: string;
  readonly proofTone: ProjectTone;
  readonly items: readonly EgressProfileStatusSummaryItem[];
  readonly warnings: readonly string[];
};

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function createNewEgressProfileEditDraft(): EgressProfileEditDraft {
  return {
    name: '',
    description: '',
    labelsText: '',
    state: 'ready',
    projectBinding: 'owner',
    projectId: '',
    proxyEnabled: false,
    proxyUrl: '',
    proxyCredentialBindingId: '',
    bypassRulesText: '',
    customCaEnabled: false,
    customCaCertificateRef: '',
    customCaDisplayName: '',
    observationMode: 'metadata_only',
    sensitiveLogSinkRef: '',
    sensitiveLogSinkDisplayName: '',
  };
}

export function createEgressProfileEditDraft(profile: EgressProfileResource): EgressProfileEditDraft {
  return {
    name: profile.name,
    description: profile.description ?? '',
    labelsText: formatLabels(profile.labels),
    state: profile.state,
    projectBinding: profile.project_id ? 'project' : 'owner',
    projectId: profile.project_id ?? '',
    proxyEnabled: Boolean(profile.proxy),
    proxyUrl: profile.proxy?.url ?? '',
    proxyCredentialBindingId: profile.proxy?.credential_binding_id ?? '',
    bypassRulesText: profile.bypass_rules.join('\n'),
    customCaEnabled: Boolean(profile.custom_ca),
    customCaCertificateRef: profile.custom_ca?.certificate_ref ?? '',
    customCaDisplayName: profile.custom_ca?.display_name ?? '',
    observationMode: profile.traffic_observation.mode,
    sensitiveLogSinkRef: profile.traffic_observation.sensitive_log_sink_ref ?? '',
    sensitiveLogSinkDisplayName: profile.traffic_observation.sensitive_log_sink_display_name ?? '',
  };
}

export function hasNewEgressProfileEditChanges(draft: EgressProfileEditDraft): boolean {
  return JSON.stringify(draft) !== JSON.stringify(createNewEgressProfileEditDraft());
}

export function hasEgressProfileEditChanges(
  profile: EgressProfileResource,
  draft: EgressProfileEditDraft,
): boolean {
  const validation = validateEgressProfileEdit(profile, draft);
  if (!validation.request) {
    return true;
  }
  const currentValidation = validateEgressProfileEdit(profile, createEgressProfileEditDraft(profile));
  return JSON.stringify(validation.request) !== JSON.stringify(currentValidation.request);
}

export function validateEgressProfileEdit(
  profile: EgressProfileResource | null,
  draft: EgressProfileEditDraft,
): EgressProfileEditValidation {
  void profile;
  const errors: string[] = [];
  const fieldErrors: Partial<Record<EgressProfileEditValidationField, string[]>> = {};
  const addError = (field: EgressProfileEditValidationField, message: string): void => {
    if (!errors.includes(message)) {
      errors.push(message);
    }
    fieldErrors[field] ??= [];
    fieldErrors[field].push(message);
  };

  const name = draft.name.trim();
  if (!name) {
    addError('name', 'Egress profile name is required.');
  }

  const projectId = draft.projectBinding === 'project' ? draft.projectId.trim() : '';
  if (draft.projectBinding === 'project' && !projectId) {
    addError('projectId', 'Select a project or switch the profile to owner scoped.');
  }

  const labelsResult = parseLabels(draft.labelsText);
  for (const error of labelsResult.errors) {
    addError('labels', error);
  }

  const bypassRulesResult = parseLines(draft.bypassRulesText, 'Bypass rule');
  for (const error of bypassRulesResult.errors) {
    addError('bypassRules', error);
  }

  const tlsIntercept = draft.observationMode === 'tls_intercept';
  const proxyRequired = draft.proxyEnabled || tlsIntercept;
  const proxyUrl = draft.proxyUrl.trim();
  const proxyCredentialBindingId = draft.proxyCredentialBindingId.trim();
  if (proxyRequired) {
    if (!proxyUrl) {
      addError('proxyUrl', 'Proxy URL is required.');
    } else {
      validateProxyUrl(proxyUrl, addError);
    }
  }
  if (proxyCredentialBindingId && !UUID_PATTERN.test(proxyCredentialBindingId)) {
    addError('proxyCredentialBindingId', 'Proxy credential binding id must be a UUID.');
  }

  const customCaRequired = draft.customCaEnabled || tlsIntercept;
  const customCaCertificateRef = draft.customCaCertificateRef.trim();
  const customCaDisplayName = draft.customCaDisplayName.trim();
  if (customCaRequired && !customCaCertificateRef) {
    addError('customCaCertificateRef', 'Custom CA certificate reference is required.');
  }
  if (draft.customCaDisplayName && !customCaDisplayName) {
    addError('customCaDisplayName', 'Custom CA display name must not be blank.');
  }

  const sensitiveLogSinkRef = draft.sensitiveLogSinkRef.trim();
  const sensitiveLogSinkDisplayName = draft.sensitiveLogSinkDisplayName.trim();
  if (tlsIntercept && !sensitiveLogSinkRef) {
    addError('sensitiveLogSinkRef', 'TLS intercept requires an approved sensitive log sink reference.');
  }
  if (sensitiveLogSinkRef && referenceContainsInlineCredentials(sensitiveLogSinkRef)) {
    addError('sensitiveLogSinkRef', 'Sensitive log sink reference must not contain inline credentials.');
  }
  if (draft.sensitiveLogSinkDisplayName && !sensitiveLogSinkDisplayName) {
    addError('sensitiveLogSinkDisplayName', 'Sensitive log sink display name must not be blank.');
  }

  if (errors.length > 0) {
    return {
      valid: false,
      errors,
      fieldErrors,
      request: null,
    };
  }

  return {
    valid: true,
    errors: [],
    fieldErrors: {},
    request: {
      project_id: projectId || null,
      name,
      description: draft.description.trim() || null,
      labels: labelsResult.labels,
      proxy: proxyRequired
        ? {
            url: proxyUrl,
            credential_binding_id: proxyCredentialBindingId || null,
          }
        : null,
      bypass_rules: bypassRulesResult.lines,
      custom_ca: customCaRequired
        ? {
            certificate_ref: customCaCertificateRef,
            display_name: customCaDisplayName || null,
          }
        : null,
      traffic_observation: {
        mode: draft.observationMode,
        sensitive_log_sink_ref: sensitiveLogSinkRef || null,
        sensitive_log_sink_display_name: sensitiveLogSinkDisplayName || null,
      },
      state: draft.state,
    },
  };
}

export function buildEgressProfileStatusSummaryModel(
  profile: EgressProfileResource,
): EgressProfileStatusSummaryModel {
  const diagnostics = profile.diagnostics;
  return {
    profileId: profile.id,
    stateLabel: profile.state,
    stateTone: profile.state === 'ready' ? 'success' : 'neutral',
    healthLabel: diagnostics.health.replaceAll('_', ' '),
    healthTone: egressHealthTone(diagnostics.health),
    proofLabel: egressProofLabel(diagnostics.proof_level),
    proofTone: diagnostics.proof_level === 'none' ? 'neutral' : 'success',
    items: [
      { label: 'Scope', value: profile.project?.name ?? profile.project_id ?? 'Owner scoped', tone: 'neutral' },
      { label: 'Proxy', value: profile.effective.proxy_configured ? 'configured' : 'none', tone: profile.effective.proxy_configured ? 'success' : 'neutral' },
      { label: 'Proxy auth', value: profile.effective.proxy_auth_configured ? 'configured' : 'none', tone: profile.effective.proxy_auth_configured ? 'success' : 'neutral' },
      { label: 'Bypass rules', value: String(profile.effective.bypass_rule_count), tone: 'neutral' },
      { label: 'Observation', value: profile.effective.observation_mode, tone: profile.effective.tls_interception_enabled ? 'warning' : 'neutral' },
      { label: 'Custom CA', value: profile.effective.custom_ca_configured ? 'configured' : 'none', tone: profile.effective.custom_ca_configured ? 'success' : 'neutral' },
      { label: 'Log sink', value: profile.effective.sensitive_log_sink_configured ? 'configured' : 'none', tone: profile.effective.sensitive_log_sink_configured ? 'success' : 'neutral' },
      { label: 'Updated', value: formatDateTime(profile.updated_at), tone: 'neutral' },
    ],
    warnings: diagnostics.warnings,
  };
}

export function mergeProjectsWithSelected(
  projects: readonly EgressProfileProjectResource[],
  selectedProjectId: string,
): readonly EgressProfileProjectResource[] {
  const sorted = [...projects].sort((left, right) => left.name.localeCompare(right.name));
  if (!selectedProjectId || sorted.some((project) => project.id === selectedProjectId)) {
    return sorted;
  }
  return sorted.concat({
    id: selectedProjectId,
    name: `Missing project ${shortId(selectedProjectId)}`,
    state: 'archived',
  });
}

export function formatLabels(labels: Readonly<Record<string, string>>): string {
  return Object.entries(labels)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}=${value}`)
    .join('\n');
}

export function setEgressObservationMode(
  draft: EgressProfileEditDraft,
  observationMode: EgressTrafficObservationMode,
): EgressProfileEditDraft {
  return {
    ...draft,
    observationMode,
    proxyEnabled: observationMode === 'tls_intercept' ? true : draft.proxyEnabled,
    customCaEnabled: observationMode === 'tls_intercept' ? true : draft.customCaEnabled,
  };
}

function validateProxyUrl(
  url: string,
  addError: (field: EgressProfileEditValidationField, message: string) => void,
): void {
  if (!(url.startsWith('http://') || url.startsWith('https://'))) {
    addError('proxyUrl', 'Proxy URL must start with http:// or https://.');
    return;
  }
  const authority = url.split('://')[1]?.split(/[/?#]/)[0] ?? '';
  if (!authority) {
    addError('proxyUrl', 'Proxy URL must include an authority.');
    return;
  }
  if (authority.includes('@')) {
    addError('proxyUrl', 'Proxy URL must not contain inline credentials.');
  }
}

function parseLabels(value: string): { readonly labels: Readonly<Record<string, string>>; readonly errors: readonly string[] } {
  const labels: Record<string, string> = {};
  const errors: string[] = [];
  value.split(/\r?\n/).forEach((rawLine, index) => {
    const line = rawLine.trim();
    if (!line) {
      return;
    }
    const separatorIndex = line.indexOf('=');
    if (separatorIndex <= 0) {
      errors.push(`Label line ${index + 1} must use key=value syntax.`);
      return;
    }
    const key = line.slice(0, separatorIndex).trim();
    const labelValue = line.slice(separatorIndex + 1).trim();
    if (!key || !labelValue) {
      errors.push(`Label line ${index + 1} must include a non-empty key and value.`);
      return;
    }
    labels[key] = labelValue;
  });
  return { labels, errors };
}

function parseLines(value: string, label: string): { readonly lines: readonly string[]; readonly errors: readonly string[] } {
  const errors: string[] = [];
  const lines = value.split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  for (const line of lines) {
    if (line.includes('\r') || line.includes('\n')) {
      errors.push(`${label} must be a single line.`);
    }
  }
  return {
    lines: [...new Set(lines)].sort((left, right) => left.localeCompare(right)),
    errors,
  };
}

function referenceContainsInlineCredentials(value: string): boolean {
  return value
    .split('://')[1]
    ?.split(/[/?#]/)[0]
    ?.includes('@') ?? false;
}

function egressHealthTone(health: EgressProfileResource['diagnostics']['health']): ProjectTone {
  if (health === 'ready') {
    return 'success';
  }
  if (health === 'attention' || health === 'unknown') {
    return 'warning';
  }
  return 'danger';
}

function egressProofLabel(proofLevel: EgressProfileResource['diagnostics']['proof_level']): string {
  if (proofLevel === 'active_probe') {
    return 'Active probe';
  }
  if (proofLevel === 'runtime_launch_metadata') {
    return 'Runtime proof';
  }
  if (proofLevel === 'configuration') {
    return 'Configuration proof';
  }
  return 'No proof';
}

function shortId(value: string): string {
  return value.length <= 12 ? value : `${value.slice(0, 8)}...`;
}
