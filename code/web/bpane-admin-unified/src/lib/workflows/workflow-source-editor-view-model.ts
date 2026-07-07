import type {
  CreateWorkflowDefinitionVersionRequest,
  ValidateWorkflowDefinitionSourceRequest,
  WorkflowDefinitionVersionResource,
} from './workflow-types';

export type WorkflowSourceEditorDraft = {
  version: string;
  executor: string;
  repositoryUrl: string;
  ref: string;
  rootPath: string;
  entrypoint: string;
};

export type WorkflowSourceEditorValidation = {
  readonly valid: boolean;
  readonly fieldErrors: Partial<Record<keyof WorkflowSourceEditorDraft, readonly string[]>>;
  readonly request: CreateWorkflowDefinitionVersionRequest | null;
  readonly sourceRequest: ValidateWorkflowDefinitionSourceRequest | null;
};

export function createWorkflowSourceEditorDraft(
  versions: readonly WorkflowDefinitionVersionResource[],
  baseVersion: WorkflowDefinitionVersionResource | null,
): WorkflowSourceEditorDraft {
  return {
    version: nextWorkflowVersion(versions),
    executor: baseVersion?.executor || 'playwright',
    repositoryUrl: baseVersion?.source?.repository_url || '/workspace',
    ref: baseVersion?.source?.ref || 'HEAD',
    rootPath: baseVersion?.source?.root_path || 'dev',
    entrypoint: baseVersion?.entrypoint || '',
  };
}

export function validateWorkflowSourceEditorDraft(
  draft: WorkflowSourceEditorDraft,
  versions: readonly WorkflowDefinitionVersionResource[],
): WorkflowSourceEditorValidation {
  const fieldErrors: Partial<Record<keyof WorkflowSourceEditorDraft, readonly string[]>> = {};
  const version = draft.version.trim();
  const executor = draft.executor.trim();
  const repositoryUrl = draft.repositoryUrl.trim();
  const ref = draft.ref.trim();
  const rootPath = draft.rootPath.trim();
  const entrypoint = draft.entrypoint.trim();

  if (!version) {
    fieldErrors.version = ['Version is required.'];
  } else if (versions.some((item) => item.version === version)) {
    fieldErrors.version = ['Version already exists. Create a new immutable version instead.'];
  }
  if (!executor) {
    fieldErrors.executor = ['Executor is required.'];
  }
  if (!repositoryUrl) {
    fieldErrors.repositoryUrl = ['Repository URL is required.'];
  }
  if (!ref) {
    fieldErrors.ref = ['Git ref is required for source validation.'];
  }
  if (!entrypoint) {
    fieldErrors.entrypoint = ['Entrypoint is required.'];
  }

  const valid = Object.keys(fieldErrors).length === 0;
  if (!valid) {
    return { valid, fieldErrors, request: null, sourceRequest: null };
  }

  const source = {
    kind: 'git' as const,
    repository_url: repositoryUrl,
    ref,
    resolved_commit: null,
    root_path: rootPath || null,
  };
  const sourceRequest = {
    entrypoint,
    source,
  };
  const request = {
    version,
    executor,
    entrypoint,
    source,
    allowed_credential_binding_ids: [],
    allowed_extension_ids: [],
    allowed_file_workspace_ids: [],
  };
  return { valid, fieldErrors, request, sourceRequest };
}

export function workflowSourceEditorRequestKey(request: ValidateWorkflowDefinitionSourceRequest | null): string {
  return request ? JSON.stringify(request) : '';
}

function nextWorkflowVersion(versions: readonly WorkflowDefinitionVersionResource[]): string {
  const numericVersions = versions
    .map((version) => /^v(\d+)$/.exec(version.version)?.[1])
    .filter((value): value is string => Boolean(value))
    .map((value) => Number.parseInt(value, 10))
    .filter((value) => Number.isFinite(value));
  const next = numericVersions.length ? Math.max(...numericVersions) + 1 : versions.length + 1;
  return `v${next}`;
}
