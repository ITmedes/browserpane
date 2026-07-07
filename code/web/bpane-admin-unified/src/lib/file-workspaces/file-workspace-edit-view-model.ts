import type {
  CreateFileWorkspaceRequest,
  FileWorkspaceProjectResource,
  FileWorkspaceResource,
} from './file-workspace-types';

export type FileWorkspaceEditValidationField =
  | 'name'
  | 'projectId'
  | 'labels';

export type FileWorkspaceProjectBinding = 'owner' | 'project';

export type FileWorkspaceEditDraft = {
  name: string;
  description: string;
  labelsText: string;
  projectBinding: FileWorkspaceProjectBinding;
  projectId: string;
};

export type FileWorkspaceEditValidationResult = {
  readonly valid: boolean;
  readonly errors: readonly string[];
  readonly fieldErrors: Partial<Record<FileWorkspaceEditValidationField, readonly string[]>>;
  readonly request: CreateFileWorkspaceRequest | null;
};

export type FileWorkspaceStatusSummaryModel = {
  readonly workspaceId: string;
  readonly items: readonly {
    readonly label: string;
    readonly value: string;
  }[];
};

export function createNewFileWorkspaceEditDraft(): FileWorkspaceEditDraft {
  return {
    name: '',
    description: '',
    labelsText: '',
    projectBinding: 'owner',
    projectId: '',
  };
}

export function hasNewFileWorkspaceEditChanges(draft: FileWorkspaceEditDraft): boolean {
  const initial = createNewFileWorkspaceEditDraft();
  return JSON.stringify(normalizedDraft(draft)) !== JSON.stringify(normalizedDraft(initial));
}

export function validateFileWorkspaceEdit(draft: FileWorkspaceEditDraft): FileWorkspaceEditValidationResult {
  const errors: string[] = [];
  const fieldErrors: Partial<Record<FileWorkspaceEditValidationField, string[]>> = {};
  const addError = (field: FileWorkspaceEditValidationField, message: string): void => {
    errors.push(message);
    fieldErrors[field] = [...(fieldErrors[field] ?? []), message];
  };

  const name = draft.name.trim();
  if (!name) {
    addError('name', 'Name is required.');
  }

  const projectId = draft.projectBinding === 'project' ? draft.projectId.trim() : '';
  if (draft.projectBinding === 'project' && !projectId) {
    addError('projectId', 'Project-scoped workspaces need a project.');
  }

  const labelsResult = parseLabels(draft.labelsText);
  for (const error of labelsResult.errors) {
    addError('labels', error);
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
    },
  };
}

export function mergeProjectsWithSelected(
  projects: readonly FileWorkspaceProjectResource[],
  selectedProjectId: string,
): readonly FileWorkspaceProjectResource[] {
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

export function buildFileWorkspaceStatusSummaryModel(
  workspace: FileWorkspaceResource,
  fileCount: number,
): FileWorkspaceStatusSummaryModel {
  return {
    workspaceId: workspace.id,
    items: [
      { label: 'Scope', value: workspace.project?.name ?? workspace.project_id ?? 'Owner scoped' },
      { label: 'Files', value: fileCount === 1 ? '1 file' : `${fileCount} files` },
      { label: 'Labels', value: Object.keys(workspace.labels).length === 1 ? '1 label' : `${Object.keys(workspace.labels).length} labels` },
      { label: 'Updated', value: workspace.updated_at },
    ],
  };
}

function normalizedDraft(draft: FileWorkspaceEditDraft): FileWorkspaceEditDraft {
  return {
    ...draft,
    name: draft.name.trim(),
    description: draft.description.trim(),
    labelsText: draft.labelsText.trim(),
    projectId: draft.projectId.trim(),
  };
}

function parseLabels(value: string): { readonly labels: Readonly<Record<string, string>>; readonly errors: readonly string[] } {
  const labels: Record<string, string> = {};
  const errors: string[] = [];
  value.split(/\r?\n|,/).forEach((rawPart, index) => {
    const part = rawPart.trim();
    if (!part) {
      return;
    }
    const separatorIndex = part.indexOf('=');
    if (separatorIndex <= 0) {
      errors.push(`Label entry ${index + 1} must use key=value syntax.`);
      return;
    }
    const key = part.slice(0, separatorIndex).trim();
    const labelValue = part.slice(separatorIndex + 1).trim();
    if (!key || !labelValue) {
      errors.push(`Label entry ${index + 1} must include a non-empty key and value.`);
      return;
    }
    labels[key] = labelValue;
  });
  return { labels, errors };
}
