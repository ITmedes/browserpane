import type {
  ProjectPolicy,
  ProjectPolicyOption,
  ProjectPolicyOptions,
  ProjectQuotas,
  ProjectResource,
  ProjectState,
  ProjectUsageBudgetEnforcement,
  UpsertProjectRequest,
} from './project-types';

export type ProjectQuotaLimitDraft = {
  enabled: boolean;
  value: string;
};

export type ProjectQuotaLimitDraftKey =
  | 'maxActiveSessions'
  | 'maxActiveWorkflowRuns'
  | 'maxRetainedStorageBytes'
  | 'maxSessionCreations'
  | 'maxSessionCreationsPerWindow'
  | 'sessionCreationWindowSec'
  | 'maxRuntimeUsageMs'
  | 'maxEgressTotalBytes';

export type ProjectPolicyBooleanDraftKey =
  | 'allowBrowserUploads'
  | 'allowBrowserDownloads'
  | 'allowSessionFileBindings'
  | 'allowManualRecordings';

export type ProjectPolicyRestrictionDraftKey =
  | 'restrictSessionTemplates'
  | 'restrictBrowserContexts'
  | 'restrictEgressProfiles'
  | 'restrictExtensions'
  | 'restrictFileWorkspaces';

export type ProjectPolicyAllowedIdsDraftKey =
  | 'allowedSessionTemplateIds'
  | 'allowedBrowserContextIds'
  | 'allowedEgressProfileIds'
  | 'allowedExtensionIds'
  | 'allowedFileWorkspaceIds';

export type ProjectPolicyOptionsKey = keyof ProjectPolicyOptions;
export type ProjectEditValidationField =
  | 'name'
  | 'labels'
  | 'usageBudgetEnforcement'
  | ProjectQuotaLimitDraftKey
  | ProjectPolicyAllowedIdsDraftKey;
export type ProjectEditFieldErrors = Partial<Record<ProjectEditValidationField, readonly string[]>>;

export type ProjectEditDraft = {
  name: string;
  description: string;
  labelsText: string;
  state: ProjectState;
  allowBrowserUploads: boolean;
  allowBrowserDownloads: boolean;
  allowSessionFileBindings: boolean;
  allowManualRecordings: boolean;
  usageBudgetEnforcement: ProjectUsageBudgetEnforcement;
  restrictSessionTemplates: boolean;
  allowedSessionTemplateIds: string[];
  restrictBrowserContexts: boolean;
  allowedBrowserContextIds: string[];
  restrictEgressProfiles: boolean;
  allowedEgressProfileIds: string[];
  restrictExtensions: boolean;
  allowedExtensionIds: string[];
  restrictFileWorkspaces: boolean;
  allowedFileWorkspaceIds: string[];
  maxActiveSessions: ProjectQuotaLimitDraft;
  maxActiveWorkflowRuns: ProjectQuotaLimitDraft;
  maxRetainedStorageBytes: ProjectQuotaLimitDraft;
  maxSessionCreations: ProjectQuotaLimitDraft;
  maxSessionCreationsPerWindow: ProjectQuotaLimitDraft;
  sessionCreationWindowSec: ProjectQuotaLimitDraft;
  maxRuntimeUsageMs: ProjectQuotaLimitDraft;
  maxEgressTotalBytes: ProjectQuotaLimitDraft;
};

export type ProjectEditValidation = {
  readonly valid: boolean;
  readonly errors: readonly string[];
  readonly fieldErrors: ProjectEditFieldErrors;
  readonly request: UpsertProjectRequest | null;
};

export type ProjectQuotaLimitDefinition = {
  readonly key: ProjectQuotaLimitDraftKey;
  readonly label: string;
  readonly description: string;
  readonly unitLabel: string;
  readonly testId: string;
  readonly presets?: readonly ProjectQuotaPresetDefinition[];
};

export type ProjectRollingSessionCreationQuotaDefinition = {
  readonly limitKey: 'maxSessionCreationsPerWindow';
  readonly windowKey: 'sessionCreationWindowSec';
  readonly label: string;
  readonly description: string;
  readonly limitLabel: string;
  readonly windowLabel: string;
  readonly testId: string;
  readonly presets: readonly ProjectRollingSessionCreationQuotaPresetDefinition[];
};

export type ProjectQuotaPresetDefinition = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type ProjectRollingSessionCreationQuotaPresetDefinition = {
  readonly label: string;
  readonly limitValue: string;
  readonly windowValue: string;
  readonly testId: string;
};

export type ProjectPolicyBooleanDefinition = {
  readonly key: ProjectPolicyBooleanDraftKey;
  readonly label: string;
  readonly description: string;
  readonly testId: string;
};

export type ProjectPolicyAllowListGroup = {
  readonly label: string;
  readonly description: string;
  readonly restrictedKey: ProjectPolicyRestrictionDraftKey;
  readonly selectedIdsKey: ProjectPolicyAllowedIdsDraftKey;
  readonly optionsKey: ProjectPolicyOptionsKey;
  readonly testId: string;
};

const BUDGET_ENFORCEMENT_VALUES = [
  'warning_only',
  'block_session_creation',
] satisfies readonly ProjectUsageBudgetEnforcement[];

const EMPTY_QUOTA_DRAFT = { enabled: false, value: '' } satisfies ProjectQuotaLimitDraft;

export const PROJECT_QUOTA_LIMITS = [
  {
    key: 'maxActiveSessions',
    label: 'Active sessions',
    description: 'Maximum simultaneously active browser sessions in this project.',
    unitLabel: 'sessions',
    testId: 'max-active-sessions',
    presets: [
      { label: '1', value: '1', testId: '1' },
      { label: '2', value: '2', testId: '2' },
      { label: '5', value: '5', testId: '5' },
      { label: '10', value: '10', testId: '10' },
    ],
  },
  {
    key: 'maxActiveWorkflowRuns',
    label: 'Active workflow runs',
    description: 'Maximum simultaneously active workflow runs in this project.',
    unitLabel: 'runs',
    testId: 'max-active-workflow-runs',
    presets: [
      { label: '1', value: '1', testId: '1' },
      { label: '2', value: '2', testId: '2' },
      { label: '5', value: '5', testId: '5' },
      { label: '10', value: '10', testId: '10' },
    ],
  },
  {
    key: 'maxRetainedStorageBytes',
    label: 'Retained storage',
    description: 'Retained bytes for workflow outputs, recordings, session files, and project workspaces.',
    unitLabel: 'bytes',
    testId: 'max-retained-storage-bytes',
    presets: [
      { label: '1 GB', value: '1073741824', testId: '1gb' },
      { label: '10 GB', value: '10737418240', testId: '10gb' },
      { label: '100 GB', value: '107374182400', testId: '100gb' },
    ],
  },
  {
    key: 'maxSessionCreations',
    label: 'Total session creations',
    description: 'Project-wide created-session budget. Generated usage alerts use this limit.',
    unitLabel: 'sessions',
    testId: 'max-session-creations',
    presets: [
      { label: '100', value: '100', testId: '100' },
      { label: '1k', value: '1000', testId: '1000' },
      { label: '10k', value: '10000', testId: '10000' },
    ],
  },
  {
    key: 'maxRuntimeUsageMs',
    label: 'Runtime budget',
    description: 'Aggregated finalized plus live runtime budget. Generated usage alerts use this limit.',
    unitLabel: 'milliseconds',
    testId: 'max-runtime-usage-ms',
    presets: [
      { label: '1 h', value: '3600000', testId: '1h' },
      { label: '4 h', value: '14400000', testId: '4h' },
      { label: '8 h', value: '28800000', testId: '8h' },
    ],
  },
  {
    key: 'maxEgressTotalBytes',
    label: 'Egress budget',
    description: 'Sanitized egress byte-counter budget. Generated alerts are advisory for this limit.',
    unitLabel: 'bytes',
    testId: 'max-egress-total-bytes',
    presets: [
      { label: '1 GB', value: '1073741824', testId: '1gb' },
      { label: '10 GB', value: '10737418240', testId: '10gb' },
      { label: '100 GB', value: '107374182400', testId: '100gb' },
    ],
  },
] satisfies readonly ProjectQuotaLimitDefinition[];

export const PROJECT_ROLLING_SESSION_CREATION_QUOTA = {
  limitKey: 'maxSessionCreationsPerWindow',
  windowKey: 'sessionCreationWindowSec',
  label: 'Session creation rate',
  description: 'Rolling-window created-session budget. The API persists this as a session count plus a window duration.',
  limitLabel: 'sessions per window',
  windowLabel: 'window seconds',
  testId: 'session-creation-rate',
  presets: [
    { label: '10 / h', limitValue: '10', windowValue: '3600', testId: '10-hour' },
    { label: '50 / h', limitValue: '50', windowValue: '3600', testId: '50-hour' },
    { label: '500 / d', limitValue: '500', windowValue: '86400', testId: '500-day' },
  ],
} satisfies ProjectRollingSessionCreationQuotaDefinition;

export const PROJECT_POLICY_BOOLEAN_FIELDS = [
  {
    key: 'allowBrowserUploads',
    label: 'Browser uploads',
    description: 'Allow live browser upload transfers for project sessions.',
    testId: 'browser-uploads',
  },
  {
    key: 'allowBrowserDownloads',
    label: 'Browser downloads',
    description: 'Allow live browser download transfers for project sessions.',
    testId: 'browser-downloads',
  },
  {
    key: 'allowSessionFileBindings',
    label: 'Session file bindings',
    description: 'Allow owners to attach workspace files as session-file bindings.',
    testId: 'session-file-bindings',
  },
  {
    key: 'allowManualRecordings',
    label: 'Manual recordings',
    description: 'Allow owners to start ad-hoc recordings for project sessions.',
    testId: 'manual-recordings',
  },
] satisfies readonly ProjectPolicyBooleanDefinition[];

export const PROJECT_POLICY_ALLOW_LIST_GROUPS = [
  {
    label: 'Session Templates',
    description: 'Restrict which templates may be used when sessions are created for this project.',
    restrictedKey: 'restrictSessionTemplates',
    selectedIdsKey: 'allowedSessionTemplateIds',
    optionsKey: 'sessionTemplates',
    testId: 'session-templates',
  },
  {
    label: 'Browser Contexts',
    description: 'Restrict reusable contexts available to this project. Fresh and ephemeral contexts remain allowed.',
    restrictedKey: 'restrictBrowserContexts',
    selectedIdsKey: 'allowedBrowserContextIds',
    optionsKey: 'browserContexts',
    testId: 'browser-contexts',
  },
  {
    label: 'Egress Profiles',
    description: 'Restrict which egress profiles may be assigned to project sessions.',
    restrictedKey: 'restrictEgressProfiles',
    selectedIdsKey: 'allowedEgressProfileIds',
    optionsKey: 'egressProfiles',
    testId: 'egress-profiles',
  },
  {
    label: 'Extensions',
    description: 'Restrict approved browser extension definitions for project sessions.',
    restrictedKey: 'restrictExtensions',
    selectedIdsKey: 'allowedExtensionIds',
    optionsKey: 'extensions',
    testId: 'extensions',
  },
  {
    label: 'File Workspaces',
    description: 'Restrict workspaces available to project workflows and session-file bindings.',
    restrictedKey: 'restrictFileWorkspaces',
    selectedIdsKey: 'allowedFileWorkspaceIds',
    optionsKey: 'fileWorkspaces',
    testId: 'file-workspaces',
  },
] satisfies readonly ProjectPolicyAllowListGroup[];

export function createNewProjectEditDraft(): ProjectEditDraft {
  return {
    name: '',
    description: '',
    labelsText: '',
    state: 'active',
    allowBrowserUploads: true,
    allowBrowserDownloads: true,
    allowSessionFileBindings: true,
    allowManualRecordings: true,
    usageBudgetEnforcement: 'warning_only',
    restrictSessionTemplates: false,
    allowedSessionTemplateIds: [],
    restrictBrowserContexts: false,
    allowedBrowserContextIds: [],
    restrictEgressProfiles: false,
    allowedEgressProfileIds: [],
    restrictExtensions: false,
    allowedExtensionIds: [],
    restrictFileWorkspaces: false,
    allowedFileWorkspaceIds: [],
    maxActiveSessions: { ...EMPTY_QUOTA_DRAFT },
    maxActiveWorkflowRuns: { ...EMPTY_QUOTA_DRAFT },
    maxRetainedStorageBytes: { ...EMPTY_QUOTA_DRAFT },
    maxSessionCreations: { ...EMPTY_QUOTA_DRAFT },
    maxSessionCreationsPerWindow: { ...EMPTY_QUOTA_DRAFT },
    sessionCreationWindowSec: { ...EMPTY_QUOTA_DRAFT },
    maxRuntimeUsageMs: { ...EMPTY_QUOTA_DRAFT },
    maxEgressTotalBytes: { ...EMPTY_QUOTA_DRAFT },
  };
}

export function createProjectEditDraft(project: ProjectResource): ProjectEditDraft {
  const { policy, quotas } = project;
  return {
    name: project.name,
    description: project.description ?? '',
    labelsText: formatLabels(project.labels),
    state: project.state,
    allowBrowserUploads: policy.allow_browser_uploads,
    allowBrowserDownloads: policy.allow_browser_downloads,
    allowSessionFileBindings: policy.allow_session_file_bindings,
    allowManualRecordings: policy.allow_manual_recordings,
    usageBudgetEnforcement: policy.usage_budget_enforcement,
    restrictSessionTemplates: policy.allowed_session_template_ids.length > 0,
    allowedSessionTemplateIds: [...policy.allowed_session_template_ids],
    restrictBrowserContexts: policy.allowed_browser_context_ids.length > 0,
    allowedBrowserContextIds: [...policy.allowed_browser_context_ids],
    restrictEgressProfiles: policy.allowed_egress_profile_ids.length > 0,
    allowedEgressProfileIds: [...policy.allowed_egress_profile_ids],
    restrictExtensions: policy.allowed_extension_ids.length > 0,
    allowedExtensionIds: [...policy.allowed_extension_ids],
    restrictFileWorkspaces: policy.allowed_file_workspace_ids.length > 0,
    allowedFileWorkspaceIds: [...policy.allowed_file_workspace_ids],
    maxActiveSessions: quotaDraft(quotas.max_active_sessions),
    maxActiveWorkflowRuns: quotaDraft(quotas.max_active_workflow_runs),
    maxRetainedStorageBytes: quotaDraft(quotas.max_retained_storage_bytes),
    maxSessionCreations: quotaDraft(quotas.max_session_creations),
    maxSessionCreationsPerWindow: quotaDraft(quotas.max_session_creations_per_window),
    sessionCreationWindowSec: quotaDraft(quotas.session_creation_window_sec),
    maxRuntimeUsageMs: quotaDraft(quotas.max_runtime_usage_ms),
    maxEgressTotalBytes: quotaDraft(quotas.max_egress_total_bytes),
  };
}

export function hasNewProjectEditChanges(draft: ProjectEditDraft): boolean {
  return JSON.stringify(draft) !== JSON.stringify(createNewProjectEditDraft());
}

export function hasProjectEditChanges(project: ProjectResource, draft: ProjectEditDraft): boolean {
  const validation = validateProjectEdit(project, draft);
  if (!validation.request) {
    return true;
  }
  const currentValidation = validateProjectEdit(project, createProjectEditDraft(project));
  return JSON.stringify(validation.request) !== JSON.stringify(currentValidation.request);
}

export function validateProjectEdit(project: ProjectResource | null, draft: ProjectEditDraft): ProjectEditValidation {
  const errors: string[] = [];
  const fieldErrors: Partial<Record<ProjectEditValidationField, string[]>> = {};
  const addError = (field: ProjectEditValidationField, message: string): void => {
    if (!errors.includes(message)) {
      errors.push(message);
    }
    fieldErrors[field] ??= [];
    fieldErrors[field].push(message);
  };
  const name = draft.name.trim();
  if (!name) {
    addError('name', 'Project name is required.');
  }
  if (!BUDGET_ENFORCEMENT_VALUES.includes(draft.usageBudgetEnforcement)) {
    addError('usageBudgetEnforcement', 'Budget enforcement has an unsupported value.');
  }
  const labelsResult = parseLabels(draft.labelsText);
  for (const error of labelsResult.errors) {
    addError('labels', error);
  }

  const quotas = projectQuotasFromDraft(draft, addError);
  const policy = projectPolicyFromDraft(draft, addError);

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
      name,
      description: draft.description.trim() || null,
      labels: labelsResult.labels,
      quotas,
      policy,
      state: draft.state,
    },
  };
}

export function formatLabels(labels: Readonly<Record<string, string>>): string {
  return Object.entries(labels)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}=${value}`)
    .join('\n');
}

export function mergePolicyOptionsWithSelected(
  options: readonly ProjectPolicyOption[],
  selectedIds: readonly string[],
): readonly ProjectPolicyOption[] {
  const knownIds = new Set(options.map((option) => option.id));
  const missingOptions = uniqueSortedIds(selectedIds)
    .filter((id) => !knownIds.has(id))
    .map((id) => ({
      id,
      projectId: null,
      name: `Missing resource ${shortId(id)}`,
      description: id,
      state: 'missing',
      scope: null,
    }));
  return [...options].sort(compareOptions).concat(missingOptions);
}

function projectQuotasFromDraft(
  draft: ProjectEditDraft,
  addError: (field: ProjectEditValidationField, message: string) => void,
): ProjectQuotas {
  const maxActiveSessions = quotaValue('Active sessions', 'maxActiveSessions', draft.maxActiveSessions, addError);
  const maxActiveWorkflowRuns = quotaValue(
    'Active workflow runs',
    'maxActiveWorkflowRuns',
    draft.maxActiveWorkflowRuns,
    addError,
  );
  const maxRetainedStorageBytes = quotaValue(
    'Retained storage',
    'maxRetainedStorageBytes',
    draft.maxRetainedStorageBytes,
    addError,
  );
  const maxSessionCreations = quotaValue(
    'Total session creations',
    'maxSessionCreations',
    draft.maxSessionCreations,
    addError,
  );
  const maxSessionCreationsPerWindow = quotaValue(
    'Session limit',
    'maxSessionCreationsPerWindow',
    draft.maxSessionCreationsPerWindow,
    addError,
  );
  const sessionCreationWindowSec = quotaValue(
    'Window duration',
    'sessionCreationWindowSec',
    draft.sessionCreationWindowSec,
    addError,
  );
  const maxRuntimeUsageMs = quotaValue('Runtime budget', 'maxRuntimeUsageMs', draft.maxRuntimeUsageMs, addError);
  const maxEgressTotalBytes = quotaValue('Egress budget', 'maxEgressTotalBytes', draft.maxEgressTotalBytes, addError);
  if (draft.maxSessionCreationsPerWindow.enabled !== draft.sessionCreationWindowSec.enabled) {
    const message = 'Rolling session creation quota needs both a session limit and a window duration.';
    addError('maxSessionCreationsPerWindow', message);
    addError('sessionCreationWindowSec', message);
  }

  return {
    max_active_sessions: maxActiveSessions,
    max_active_workflow_runs: maxActiveWorkflowRuns,
    max_retained_storage_bytes: maxRetainedStorageBytes,
    max_session_creations: maxSessionCreations,
    max_session_creations_per_window: maxSessionCreationsPerWindow,
    session_creation_window_sec: sessionCreationWindowSec,
    max_runtime_usage_ms: maxRuntimeUsageMs,
    max_egress_total_bytes: maxEgressTotalBytes,
  };
}

function projectPolicyFromDraft(
  draft: ProjectEditDraft,
  addError: (field: ProjectEditValidationField, message: string) => void,
): ProjectPolicy {
  return {
    allowed_session_template_ids: restrictedIds(
      'Session Templates',
      'allowedSessionTemplateIds',
      draft.restrictSessionTemplates,
      draft.allowedSessionTemplateIds,
      addError,
    ),
    allowed_egress_profile_ids: restrictedIds(
      'Egress Profiles',
      'allowedEgressProfileIds',
      draft.restrictEgressProfiles,
      draft.allowedEgressProfileIds,
      addError,
    ),
    allowed_extension_ids: restrictedIds(
      'Extensions',
      'allowedExtensionIds',
      draft.restrictExtensions,
      draft.allowedExtensionIds,
      addError,
    ),
    allowed_browser_context_ids: restrictedIds(
      'Browser Contexts',
      'allowedBrowserContextIds',
      draft.restrictBrowserContexts,
      draft.allowedBrowserContextIds,
      addError,
    ),
    allowed_file_workspace_ids: restrictedIds(
      'File Workspaces',
      'allowedFileWorkspaceIds',
      draft.restrictFileWorkspaces,
      draft.allowedFileWorkspaceIds,
      addError,
    ),
    allow_browser_uploads: draft.allowBrowserUploads,
    allow_browser_downloads: draft.allowBrowserDownloads,
    allow_session_file_bindings: draft.allowSessionFileBindings,
    allow_manual_recordings: draft.allowManualRecordings,
    usage_budget_enforcement: draft.usageBudgetEnforcement,
  };
}

function quotaDraft(value: number | null | undefined): ProjectQuotaLimitDraft {
  return {
    enabled: value !== undefined && value !== null,
    value: value === undefined || value === null ? '' : String(value),
  };
}

function quotaValue(
  label: string,
  field: ProjectQuotaLimitDraftKey,
  draft: ProjectQuotaLimitDraft,
  addError: (field: ProjectEditValidationField, message: string) => void,
): number | null {
  if (!draft.enabled) {
    return null;
  }
  const value = draft.value.trim();
  if (!value) {
    addError(field, `${label} quota needs a positive integer.`);
    return null;
  }
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1) {
    addError(field, `${label} quota needs a positive integer.`);
    return null;
  }
  return parsed;
}

function restrictedIds(
  label: string,
  field: ProjectPolicyAllowedIdsDraftKey,
  restricted: boolean,
  selectedIds: readonly string[],
  addError: (field: ProjectEditValidationField, message: string) => void,
): readonly string[] {
  if (!restricted) {
    return [];
  }
  const normalized = uniqueSortedIds(selectedIds);
  if (normalized.length === 0) {
    addError(field, `${label} restriction needs at least one selected resource.`);
  }
  return normalized;
}

function uniqueSortedIds(ids: readonly string[]): string[] {
  return [...new Set(ids.map((id) => id.trim()).filter(Boolean))].sort((left, right) => left.localeCompare(right));
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
      errors.push(`Label line ${index + 1} must use key=value.`);
      return;
    }
    const key = line.slice(0, separatorIndex).trim();
    const labelValue = line.slice(separatorIndex + 1).trim();
    if (!key) {
      errors.push(`Label line ${index + 1} has an empty key.`);
      return;
    }
    if (!labelValue) {
      errors.push(`Label line ${index + 1} has an empty value.`);
      return;
    }
    if (Object.hasOwn(labels, key)) {
      errors.push(`Label "${key}" is defined more than once.`);
      return;
    }
    labels[key] = labelValue;
  });
  return { labels, errors };
}

function compareOptions(left: ProjectPolicyOption, right: ProjectPolicyOption): number {
  return left.name.localeCompare(right.name) || left.id.localeCompare(right.id);
}

function shortId(id: string): string {
  return id.length > 8 ? id.slice(0, 8) : id;
}
