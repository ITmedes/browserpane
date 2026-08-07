import type { ProjectTone } from '$lib/projects/project-formatters';
import type {
  BrowserContextPersistenceMode,
  BrowserContextProjectResource,
  BrowserContextResource,
  CreateBrowserContextRequest,
} from './browser-context-types';
import {
  retentionSummary,
  storageSummary,
} from './browser-context-overview-view-model';

export type BrowserContextEditValidationField =
  | 'name'
  | 'projectId'
  | 'labels'
  | 'retentionSec'
  | 'maxProfileStorageBytes';

export type BrowserContextProjectBinding = 'owner' | 'project';

export type BrowserContextEditDraft = {
  name: string;
  description: string;
  labelsText: string;
  projectBinding: BrowserContextProjectBinding;
  projectId: string;
  persistenceMode: BrowserContextPersistenceMode;
  retentionEnabled: boolean;
  retentionSec: string;
  storageLimitEnabled: boolean;
  maxProfileStorageBytes: string;
};

export type BrowserContextEditValidationResult = {
  readonly valid: boolean;
  readonly errors: readonly string[];
  readonly fieldErrors: Partial<Record<BrowserContextEditValidationField, readonly string[]>>;
  readonly request: CreateBrowserContextRequest | null;
};

export type BrowserContextStatusSummaryModel = {
  readonly contextId: string;
  readonly stateLabel: string;
  readonly stateTone: ProjectTone;
  readonly persistenceLabel: string;
  readonly persistenceTone: ProjectTone;
  readonly items: readonly {
    readonly label: string;
    readonly value: string;
    readonly tone: ProjectTone;
  }[];
};

export function createNewBrowserContextEditDraft(): BrowserContextEditDraft {
  return {
    name: '',
    description: '',
    labelsText: '',
    projectBinding: 'owner',
    projectId: '',
    persistenceMode: 'reusable',
    retentionEnabled: true,
    retentionSec: '604800',
    storageLimitEnabled: false,
    maxProfileStorageBytes: '',
  };
}

export function hasNewBrowserContextEditChanges(draft: BrowserContextEditDraft): boolean {
  return hasBrowserContextEditChanges(draft, createNewBrowserContextEditDraft());
}

export function hasBrowserContextEditChanges(
  draft: BrowserContextEditDraft,
  initialDraft: BrowserContextEditDraft,
): boolean {
  return JSON.stringify(normalizedDraft(draft)) !== JSON.stringify(normalizedDraft(initialDraft));
}

export function browserContextEditDraftFromResource(
  context: BrowserContextResource,
  name = context.name,
): BrowserContextEditDraft {
  return {
    name,
    description: context.description ?? '',
    labelsText: Object.entries(context.labels)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, value]) => `${key}=${value}`)
      .join('\n'),
    projectBinding: context.project_id ? 'project' : 'owner',
    projectId: context.project_id ?? '',
    persistenceMode: context.persistence_mode,
    retentionEnabled: context.retention_sec !== null && context.retention_sec !== undefined,
    retentionSec: context.retention_sec === null || context.retention_sec === undefined
      ? ''
      : String(context.retention_sec),
    storageLimitEnabled:
      context.max_profile_storage_bytes !== null && context.max_profile_storage_bytes !== undefined,
    maxProfileStorageBytes:
      context.max_profile_storage_bytes === null || context.max_profile_storage_bytes === undefined
        ? ''
        : String(context.max_profile_storage_bytes),
  };
}

export function validateBrowserContextEdit(draft: BrowserContextEditDraft): BrowserContextEditValidationResult {
  const errors: string[] = [];
  const fieldErrors: Partial<Record<BrowserContextEditValidationField, string[]>> = {};
  const addError = (field: BrowserContextEditValidationField, message: string): void => {
    errors.push(message);
    fieldErrors[field] = [...(fieldErrors[field] ?? []), message];
  };

  const name = draft.name.trim();
  if (!name) {
    addError('name', 'Name is required.');
  }

  const projectId = draft.projectBinding === 'project' ? draft.projectId.trim() : '';
  if (draft.projectBinding === 'project' && !projectId) {
    addError('projectId', 'Project-scoped contexts need a project.');
  }

  const labelsResult = parseLabels(draft.labelsText);
  for (const error of labelsResult.errors) {
    addError('labels', error);
  }

  const retentionSec = optionalPositiveInteger(
    draft.retentionEnabled,
    draft.retentionSec,
    'Retention seconds',
    'retentionSec',
    addError,
  );
  const maxProfileStorageBytes = optionalPositiveInteger(
    draft.storageLimitEnabled,
    draft.maxProfileStorageBytes,
    'Profile storage limit',
    'maxProfileStorageBytes',
    addError,
  );

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
      persistence_mode: draft.persistenceMode,
      retention_sec: retentionSec,
      max_profile_storage_bytes: maxProfileStorageBytes,
    },
  };
}

export function mergeProjectsWithSelected(
  projects: readonly BrowserContextProjectResource[],
  selectedProjectId: string,
): readonly BrowserContextProjectResource[] {
  if (!selectedProjectId || projects.some((project) => project.id === selectedProjectId)) {
    return projects;
  }
  return [
    ...projects,
    {
      id: selectedProjectId,
      name: `Missing project ${selectedProjectId.slice(0, 8)}...`,
      state: 'archived',
    },
  ];
}

export function buildBrowserContextStatusSummaryModel(
  context: BrowserContextResource,
): BrowserContextStatusSummaryModel {
  return {
    contextId: context.id,
    stateLabel: context.state,
    stateTone: context.state === 'ready' ? 'success' : 'neutral',
    persistenceLabel: context.persistence_mode,
    persistenceTone: context.persistence_mode === 'reusable' ? 'success' : 'neutral',
    items: [
      { label: 'Scope', value: context.project?.name ?? context.project_id ?? 'Owner scoped', tone: context.project_id ? 'warning' : 'neutral' },
      { label: 'Retention', value: retentionSummary(context), tone: context.retention_sec ? 'success' : 'neutral' },
      { label: 'Storage', value: storageSummary(context), tone: context.usage.profile_storage_limit_exceeded ? 'danger' : 'neutral' },
      { label: 'References', value: `${context.usage.visible_session_count} visible sessions`, tone: context.usage.visible_session_count > 0 ? 'warning' : 'neutral' },
      { label: 'Runtime', value: `${context.usage.active_runtime_session_count} active runtime`, tone: context.usage.active_runtime_session_count > 0 ? 'warning' : 'neutral' },
    ],
  };
}

function normalizedDraft(draft: BrowserContextEditDraft): BrowserContextEditDraft {
  return {
    ...draft,
    name: draft.name.trim(),
    description: draft.description.trim(),
    labelsText: draft.labelsText.trim(),
    projectId: draft.projectId.trim(),
    retentionSec: String(draft.retentionSec).trim(),
    maxProfileStorageBytes: String(draft.maxProfileStorageBytes).trim(),
  };
}

function optionalPositiveInteger(
  enabled: boolean,
  rawValue: string,
  label: string,
  field: BrowserContextEditValidationField,
  addError: (field: BrowserContextEditValidationField, message: string) => void,
): number | null {
  const value = String(rawValue).trim();
  if (!enabled) {
    return null;
  }
  if (!value) {
    addError(field, `${label} is required when enabled.`);
    return null;
  }
  if (!/^\d+$/.test(value)) {
    addError(field, `${label} must be a whole number.`);
    return null;
  }
  const numberValue = Number(value);
  if (!Number.isSafeInteger(numberValue) || numberValue <= 0) {
    addError(field, `${label} must be greater than zero.`);
    return null;
  }
  return numberValue;
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
