import { formatDateTime } from '$lib/projects/project-formatters';
import type { SessionFileBindingResource, SessionFileResource } from './session-file-types';

export type SessionMountPathValidation = {
  readonly valid: boolean;
  readonly value: string;
  readonly message: string;
};

export type SessionFileRow = {
  readonly id: string;
  readonly name: string;
  readonly source: string;
  readonly mediaType: string;
  readonly size: string;
  readonly digest: string;
  readonly createdAt: string;
};

export type SessionFileBindingRow = {
  readonly id: string;
  readonly workspaceId: string;
  readonly fileName: string;
  readonly mountPath: string;
  readonly mode: string;
  readonly state: string;
  readonly mediaType: string;
  readonly size: string;
  readonly digest: string;
  readonly error: string | null;
  readonly createdAt: string;
};

export function sessionFileRow(file: SessionFileResource): SessionFileRow {
  return {
    id: file.id,
    name: file.name,
    source: file.source.replaceAll('_', ' '),
    mediaType: file.media_type ?? 'application/octet-stream',
    size: formatBytes(file.byte_count),
    digest: shortDigest(file.sha256_hex),
    createdAt: formatDateTime(file.created_at),
  };
}

export function sessionFileBindingRow(binding: SessionFileBindingResource): SessionFileBindingRow {
  return {
    id: binding.id,
    workspaceId: binding.workspace_id,
    fileName: binding.file_name,
    mountPath: binding.mount_path,
    mode: binding.mode.replaceAll('_', ' '),
    state: binding.state.replaceAll('_', ' '),
    mediaType: binding.media_type ?? 'application/octet-stream',
    size: formatBytes(binding.byte_count),
    digest: shortDigest(binding.sha256_hex),
    error: binding.error ?? null,
    createdAt: formatDateTime(binding.created_at),
  };
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
  if (/[\u0000-\u001f\u007f]/.test(value)) {
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
  return { valid: true, value, message: 'Mount path is valid.' };
}

function invalid(value: string, message: string): SessionMountPathValidation {
  return { valid: false, value, message };
}

function formatBytes(value: number): string {
  if (value < 1024) {
    return `${value} B`;
  }
  const units = ['KB', 'MB', 'GB'];
  let amount = value / 1024;
  let unit = units[0];
  for (let index = 1; index < units.length && amount >= 1024; index += 1) {
    amount /= 1024;
    unit = units[index];
  }
  return `${amount.toFixed(amount >= 10 ? 0 : 1)} ${unit}`;
}

function shortDigest(value: string): string {
  return `sha256 ${value.length > 16 ? `${value.slice(0, 16)}...` : value}`;
}
