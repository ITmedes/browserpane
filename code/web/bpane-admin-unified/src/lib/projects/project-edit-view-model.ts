import type { ProjectResource, ProjectState, UpsertProjectRequest } from './project-types';

export type ProjectEditDraft = {
  readonly name: string;
  readonly description: string;
  readonly labelsText: string;
  readonly state: ProjectState;
};

export type ProjectEditValidation = {
  readonly valid: boolean;
  readonly errors: readonly string[];
  readonly request: UpsertProjectRequest | null;
};

export function createProjectEditDraft(project: ProjectResource): ProjectEditDraft {
  return {
    name: project.name,
    description: project.description ?? '',
    labelsText: formatLabels(project.labels),
    state: project.state,
  };
}

export function hasProjectEditChanges(project: ProjectResource, draft: ProjectEditDraft): boolean {
  const validation = validateProjectEdit(project, draft);
  if (!validation.request) {
    return true;
  }
  return JSON.stringify(validation.request) !== JSON.stringify(toCurrentProjectRequest(project));
}

export function validateProjectEdit(project: ProjectResource, draft: ProjectEditDraft): ProjectEditValidation {
  const errors: string[] = [];
  const name = draft.name.trim();
  if (!name) {
    errors.push('Project name is required.');
  }
  const labelsResult = parseLabels(draft.labelsText);
  errors.push(...labelsResult.errors);

  if (errors.length > 0) {
    return {
      valid: false,
      errors,
      request: null,
    };
  }

  return {
    valid: true,
    errors: [],
    request: {
      name,
      description: draft.description.trim() || null,
      labels: labelsResult.labels,
      quotas: project.quotas,
      policy: project.policy,
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

function toCurrentProjectRequest(project: ProjectResource): UpsertProjectRequest {
  return {
    name: project.name,
    description: project.description ?? null,
    labels: project.labels,
    quotas: project.quotas,
    policy: project.policy,
    state: project.state,
  };
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
