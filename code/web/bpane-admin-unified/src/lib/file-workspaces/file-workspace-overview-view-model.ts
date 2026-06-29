import {
  formatBytes,
  formatDateTime,
  type ProjectTone,
} from '$lib/projects/project-formatters';
import type {
  FileWorkspaceFileResource,
  FileWorkspaceResource,
} from './file-workspace-types';

export type FileWorkspaceFileCountMap = Readonly<Record<string, number | null>>;

export type FileWorkspaceOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | {
      readonly status: 'ready';
      readonly workspaces: readonly FileWorkspaceResource[];
      readonly fileCounts: FileWorkspaceFileCountMap;
    };

export type FileWorkspaceOverviewMetric = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type FileWorkspaceOverviewRow = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly scope: string;
  readonly scopeTone: ProjectTone;
  readonly fileCount: number | null;
  readonly fileCountLabel: string;
  readonly fileCountTone: ProjectTone;
  readonly labels: string;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly badges: readonly string[];
};

export type FileWorkspaceOverviewModel = {
  readonly metrics: readonly FileWorkspaceOverviewMetric[];
  readonly rows: readonly FileWorkspaceOverviewRow[];
};

export type FileWorkspaceFileRow = {
  readonly id: string;
  readonly workspaceId: string;
  readonly name: string;
  readonly mediaType: string;
  readonly size: string;
  readonly digest: string;
  readonly provenance: string;
  readonly createdAt: string;
  readonly updatedAt: string;
};

export function buildFileWorkspaceOverviewModel(
  workspaces: readonly FileWorkspaceResource[],
  fileCounts: FileWorkspaceFileCountMap = {},
): FileWorkspaceOverviewModel {
  const knownFileCount = Object.values(fileCounts)
    .filter((count): count is number => typeof count === 'number')
    .reduce((total, count) => total + count, 0);
  return {
    metrics: [
      metric('total', 'Workspaces', workspaces.length),
      metric('owner', 'Owner scoped', workspaces.filter((workspace) => !workspace.project_id).length),
      metric('project', 'Project scoped', workspaces.filter((workspace) => Boolean(workspace.project_id)).length),
      metric('files', 'Known files', knownFileCount),
    ],
    rows: workspaces.map((workspace) => fileWorkspaceRow(workspace, fileCounts[workspace.id] ?? null)),
  };
}

export function fileWorkspaceRow(
  workspace: FileWorkspaceResource,
  fileCount: number | null = null,
): FileWorkspaceOverviewRow {
  const projectScoped = Boolean(workspace.project_id);
  return {
    id: workspace.id,
    name: workspace.name,
    description: workspace.description?.trim() || workspace.id,
    scope: workspace.project?.name ?? workspace.project_id ?? 'Owner scoped',
    scopeTone: projectScoped ? 'warning' : 'neutral',
    fileCount,
    fileCountLabel: fileCount === null
      ? 'files unavailable'
      : fileCount === 1
        ? '1 file'
        : `${fileCount} files`,
    fileCountTone: fileCount === null ? 'warning' : fileCount > 0 ? 'success' : 'neutral',
    labels: labelSummary(workspace.labels),
    createdAt: formatDateTime(workspace.created_at),
    updatedAt: formatDateTime(workspace.updated_at),
    badges: [
      projectScoped ? 'project scoped' : 'owner scoped',
      fileCount === null ? 'files unavailable' : `${fileCount} files`,
      ...Object.entries(workspace.labels).map(([key, value]) => `${key}=${value}`),
    ],
  };
}

export function fileWorkspaceMatchesSearch(
  row: FileWorkspaceOverviewRow,
  normalizedQuery: string,
): boolean {
  if (!normalizedQuery) {
    return true;
  }
  return [
    row.id,
    row.name,
    row.description,
    row.scope,
    row.fileCountLabel,
    row.labels,
    row.createdAt,
    row.updatedAt,
    ...row.badges,
  ].some((value) => value.toLowerCase().includes(normalizedQuery));
}

export function fileWorkspaceFileRow(file: FileWorkspaceFileResource): FileWorkspaceFileRow {
  return {
    id: file.id,
    workspaceId: file.workspace_id,
    name: file.name,
    mediaType: file.media_type ?? 'application/octet-stream',
    size: formatBytes(file.byte_count) ?? `${file.byte_count} B`,
    digest: `sha256 ${shortDigest(file.sha256_hex)}`,
    provenance: provenanceSummary(file.provenance),
    createdAt: formatDateTime(file.created_at),
    updatedAt: formatDateTime(file.updated_at),
  };
}

export function labelSummary(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels).sort(([left], [right]) => left.localeCompare(right));
  if (entries.length === 0) {
    return 'No labels';
  }
  return entries.map(([key, value]) => `${key}=${value}`).join(', ');
}

export function provenanceSummary(provenance: Readonly<Record<string, unknown>> | null): string {
  if (!provenance) {
    return 'Provenance unavailable';
  }
  const entries = Object.entries(provenance)
    .sort(([left], [right]) => left.localeCompare(right))
    .slice(0, 4)
    .map(([key, value]) => `${key}=${stringifyMetadataValue(value)}`);
  if (entries.length === 0) {
    return 'Provenance unavailable';
  }
  return entries.join(', ');
}

function shortDigest(value: string): string {
  return value.length <= 16 ? value : `${value.slice(0, 12)}...${value.slice(-4)}`;
}

function stringifyMetadataValue(value: unknown): string {
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }
  if (value === null || value === undefined) {
    return 'unavailable';
  }
  const serialized = JSON.stringify(value);
  return serialized.length > 80 ? `${serialized.slice(0, 80)}...` : serialized;
}

function metric(key: string, label: string, value: number): FileWorkspaceOverviewMetric {
  return {
    label,
    value: String(value),
    testId: `file-workspaces-metric-${key}`,
  };
}
