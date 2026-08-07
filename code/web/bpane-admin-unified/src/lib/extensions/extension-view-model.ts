import { parseKeyValueLabels } from '$lib/application/admin-form-utils';
import { formatDateTime, type ProjectTone } from '$lib/projects/project-formatters';
import type {
  CreateExtensionDefinitionRequest,
  CreateExtensionVersionRequest,
  ExtensionDefinitionResource,
} from './extension-types';

export type ExtensionOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly extensions: readonly ExtensionDefinitionResource[] };

export type ExtensionDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly extensionId: string }
  | { readonly status: 'error'; readonly extensionId: string; readonly message: string }
  | { readonly status: 'ready'; readonly extension: ExtensionDefinitionResource };

export type ExtensionOverviewRow = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly status: string;
  readonly statusTone: ProjectTone;
  readonly latestVersion: string;
  readonly versionTone: ProjectTone;
  readonly labels: string;
  readonly updatedAt: string;
};

export type ExtensionOverviewModel = {
  readonly metrics: readonly { readonly label: string; readonly value: string; readonly testId: string }[];
  readonly rows: readonly ExtensionOverviewRow[];
};

export type ExtensionDefinitionDraft = {
  name: string;
  description: string;
  labelsText: string;
};

export type ExtensionVersionDraft = {
  version: string;
  installPath: string;
};

export type ExtensionFormValidation<T> = {
  readonly valid: boolean;
  readonly request: T | null;
  readonly fieldErrors: Readonly<Record<string, readonly string[]>>;
};

export function buildExtensionOverviewModel(
  extensions: readonly ExtensionDefinitionResource[],
): ExtensionOverviewModel {
  return {
    metrics: [
      metric('total', 'Extensions', extensions.length),
      metric('enabled', 'Enabled', extensions.filter((extension) => extension.enabled).length),
      metric('disabled', 'Disabled', extensions.filter((extension) => !extension.enabled).length),
      metric('versioned', 'Versioned', extensions.filter((extension) => extension.latest_version).length),
    ],
    rows: extensions.map(extensionOverviewRow),
  };
}

export function extensionOverviewRow(extension: ExtensionDefinitionResource): ExtensionOverviewRow {
  return {
    id: extension.id,
    name: extension.name,
    description: extension.description?.trim() || extension.id,
    status: extension.enabled ? 'Enabled' : 'Disabled',
    statusTone: extension.enabled ? 'success' : 'neutral',
    latestVersion: extension.latest_version ?? 'No version',
    versionTone: extension.latest_version ? 'success' : 'warning',
    labels: labelSummary(extension.labels),
    updatedAt: formatDateTime(extension.updated_at),
  };
}

export function extensionMatchesSearch(row: ExtensionOverviewRow, normalizedQuery: string): boolean {
  if (!normalizedQuery) {
    return true;
  }
  return [
    row.id,
    row.name,
    row.description,
    row.status,
    row.latestVersion,
    row.labels,
    row.updatedAt,
  ].some((value) => value.toLowerCase().includes(normalizedQuery));
}

export function createExtensionDefinitionDraft(): ExtensionDefinitionDraft {
  return { name: '', description: '', labelsText: '' };
}

export function createExtensionVersionDraft(): ExtensionVersionDraft {
  return { version: '', installPath: '' };
}

export function validateExtensionDefinitionDraft(
  draft: ExtensionDefinitionDraft,
): ExtensionFormValidation<CreateExtensionDefinitionRequest> {
  const fieldErrors: Record<string, string[]> = {};
  const name = draft.name.trim();
  const description = draft.description.trim();
  if (!name) {
    fieldErrors.name = ['Name is required.'];
  }
  const labels = parseKeyValueLabels(draft.labelsText);
  if (!labels.ok) {
    fieldErrors.labels = [labels.error];
  }
  const valid = Object.keys(fieldErrors).length === 0;
  return {
    valid,
    fieldErrors,
    request: valid && labels.ok
      ? { name, description: description || null, labels: labels.value }
      : null,
  };
}

export function validateExtensionVersionDraft(
  draft: ExtensionVersionDraft,
): ExtensionFormValidation<CreateExtensionVersionRequest> {
  const fieldErrors: Record<string, string[]> = {};
  const version = draft.version.trim();
  const installPath = draft.installPath.trim();
  if (!version) {
    fieldErrors.version = ['Version is required.'];
  }
  if (!installPath) {
    fieldErrors.installPath = ['Install path is required.'];
  } else if (!installPath.startsWith('/')) {
    fieldErrors.installPath = ['Install path must be absolute.'];
  }
  const valid = Object.keys(fieldErrors).length === 0;
  return {
    valid,
    fieldErrors,
    request: valid ? { version, install_path: installPath } : null,
  };
}

export function labelSummary(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels).sort(([left], [right]) => left.localeCompare(right));
  return entries.length === 0 ? 'No labels' : entries.map(([key, value]) => `${key}=${value}`).join(', ');
}

function metric(key: string, label: string, value: number) {
  return { label, value: String(value), testId: `extensions-metric-${key}` };
}
