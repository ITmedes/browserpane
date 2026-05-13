import type {
  FileWorkspaceFileResource,
  FileWorkspaceResource,
  SessionFileBindingResource,
} from '../api/control-types';
import {
  formatSessionFileBytes,
  formatSessionFileTimestamp,
  shortSessionFileDigest,
} from './session-file-format';

export type FileWorkspaceListViewModel = {
  readonly rows: readonly FileWorkspaceRowViewModel[];
  readonly totalCount: number;
  readonly emptyMessage: string;
};

export type FileWorkspaceRowViewModel = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly labels: string;
  readonly createdAt: string;
  readonly updatedAt: string;
};

export type FileWorkspaceFileViewModel = {
  readonly id: string;
  readonly workspaceId: string;
  readonly name: string;
  readonly mediaType: string;
  readonly size: string;
  readonly digest: string;
  readonly provenance: string;
  readonly createdAt: string;
};

export type SessionFileBindingViewModel = {
  readonly id: string;
  readonly sessionId: string;
  readonly workspaceId: string;
  readonly fileId: string;
  readonly fileName: string;
  readonly mountPath: string;
  readonly mediaType: string;
  readonly size: string;
  readonly digest: string;
  readonly mode: string;
  readonly state: string;
  readonly error: string;
  readonly provenance: string;
  readonly createdAt: string;
};

export type SessionMountPathValidation = {
  readonly valid: boolean;
  readonly value: string;
  readonly message: string;
};

export class FileWorkspaceViewModelBuilder {
  static list(input: {
    readonly workspaces: readonly FileWorkspaceResource[];
    readonly search: string;
  }): FileWorkspaceListViewModel {
    const normalized = input.search.trim().toLowerCase();
    const rows = input.workspaces
      .map((workspace) => this.workspaceRow(workspace))
      .filter((row) => workspaceRowMatches(row, normalized));
    return {
      rows,
      totalCount: input.workspaces.length,
      emptyMessage: normalized
        ? 'No file workspaces match the current filter.'
        : 'No file workspaces are available yet.',
    };
  }

  static workspaceRow(workspace: FileWorkspaceResource): FileWorkspaceRowViewModel {
    return {
      id: workspace.id,
      name: workspace.name,
      description: workspace.description ?? 'No description available.',
      labels: labelSummary(workspace.labels),
      createdAt: formatSessionFileTimestamp(workspace.created_at),
      updatedAt: formatSessionFileTimestamp(workspace.updated_at),
    };
  }

  static workspaceFile(file: FileWorkspaceFileResource): FileWorkspaceFileViewModel {
    return {
      id: file.id,
      workspaceId: file.workspace_id,
      name: file.name,
      mediaType: file.media_type ?? 'application/octet-stream',
      size: formatSessionFileBytes(file.byte_count),
      digest: `sha256 ${shortSessionFileDigest(file.sha256_hex)}`,
      provenance: provenanceSummary(file.provenance),
      createdAt: formatSessionFileTimestamp(file.created_at),
    };
  }

  static sessionBinding(binding: SessionFileBindingResource): SessionFileBindingViewModel {
    return {
      id: binding.id,
      sessionId: binding.session_id,
      workspaceId: binding.workspace_id,
      fileId: binding.file_id,
      fileName: binding.file_name,
      mountPath: binding.mount_path,
      mediaType: binding.media_type ?? 'application/octet-stream',
      size: formatSessionFileBytes(binding.byte_count),
      digest: `sha256 ${shortSessionFileDigest(binding.sha256_hex)}`,
      mode: binding.mode.replaceAll('_', ' '),
      state: binding.state.replaceAll('_', ' '),
      error: binding.error ?? 'No materialization error.',
      provenance: provenanceSummary(binding.provenance),
      createdAt: formatSessionFileTimestamp(binding.created_at),
    };
  }
}

export function validateSessionMountPath(
  rawValue: string,
  existingMountPaths: readonly string[] = [],
): SessionMountPathValidation {
  const value = rawValue.trim();
  if (!value) {
    return invalid(value, 'Mount path is required.');
  }
  if (value.startsWith('/')) {
    return invalid(value, 'Mount path must be relative.');
  }
  if (value.includes('\\')) {
    return invalid(value, 'Mount path must use forward slashes.');
  }
  if (value.includes('\0')) {
    return invalid(value, 'Mount path must not contain control characters.');
  }
  const parts = value.split('/');
  if (parts.some((part) => part.length === 0)) {
    return invalid(value, 'Mount path must not contain empty path components.');
  }
  if (parts.some((part) => part === '.' || part === '..')) {
    return invalid(value, 'Mount path must not contain traversal components.');
  }
  if (existingMountPaths.includes(value)) {
    return invalid(value, 'Mount path is already bound for this session.');
  }
  return {
    valid: true,
    value,
    message: 'Mount path is valid.',
  };
}

function invalid(value: string, message: string): SessionMountPathValidation {
  return { valid: false, value, message };
}

function workspaceRowMatches(row: FileWorkspaceRowViewModel, normalized: string): boolean {
  if (!normalized) {
    return true;
  }
  return [
    row.id,
    row.name,
    row.description,
    row.labels,
    row.createdAt,
    row.updatedAt,
  ].some((value) => value.toLowerCase().includes(normalized));
}

function labelSummary(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels).sort(([left], [right]) => left.localeCompare(right));
  if (entries.length === 0) {
    return 'No labels';
  }
  return entries.map(([key, value]) => `${key}=${value}`).join(', ');
}

function provenanceSummary(provenance: Readonly<Record<string, unknown>> | null): string {
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
