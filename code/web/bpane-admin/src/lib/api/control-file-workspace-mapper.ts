import type {
  FileWorkspaceFileListResponse,
  FileWorkspaceFileResource,
  FileWorkspaceListResponse,
  FileWorkspaceResource,
  SessionFileBindingListResponse,
  SessionFileBindingMode,
  SessionFileBindingResource,
  SessionFileBindingState,
} from './control-types';
import {
  expectNumber,
  expectRecord,
  expectString,
  expectStringRecord,
  optionalString,
} from './control-wire';

const SESSION_FILE_BINDING_MODES = new Set<string>(['read_only', 'read_write', 'scratch_output']);
const SESSION_FILE_BINDING_STATES = new Set<string>(['pending', 'materialized', 'failed', 'removed']);

export class ControlFileWorkspaceMapper {
  static toWorkspaceList(payload: unknown): FileWorkspaceListResponse {
    const object = expectRecord(payload, 'file workspace list response');
    const workspaces = object.workspaces;
    if (!Array.isArray(workspaces)) {
      throw new Error('file workspace list response must contain a workspaces array');
    }
    return {
      workspaces: workspaces.map((workspace) => this.toWorkspace(workspace)),
    };
  }

  static toWorkspace(payload: unknown): FileWorkspaceResource {
    const object = expectRecord(payload, 'file workspace resource');
    const description = optionalString(object.description, 'file workspace description');
    return {
      id: expectString(object.id, 'file workspace id'),
      name: expectString(object.name, 'file workspace name'),
      ...(description !== undefined ? { description } : {}),
      labels: expectStringRecord(object.labels, 'file workspace labels'),
      files_path: expectString(object.files_path, 'file workspace files_path'),
      created_at: expectString(object.created_at, 'file workspace created_at'),
      updated_at: expectString(object.updated_at, 'file workspace updated_at'),
    };
  }

  static toWorkspaceFileList(payload: unknown): FileWorkspaceFileListResponse {
    const object = expectRecord(payload, 'file workspace file list response');
    const files = object.files;
    if (!Array.isArray(files)) {
      throw new Error('file workspace file list response must contain a files array');
    }
    return {
      files: files.map((file) => this.toWorkspaceFile(file)),
    };
  }

  static toWorkspaceFile(payload: unknown): FileWorkspaceFileResource {
    const object = expectRecord(payload, 'file workspace file resource');
    const mediaType = optionalString(object.media_type, 'file workspace file media_type');
    return {
      id: expectString(object.id, 'file workspace file id'),
      workspace_id: expectString(object.workspace_id, 'file workspace file workspace_id'),
      name: expectString(object.name, 'file workspace file name'),
      ...(mediaType !== undefined ? { media_type: mediaType } : {}),
      byte_count: expectNumber(object.byte_count, 'file workspace file byte_count'),
      sha256_hex: expectString(object.sha256_hex, 'file workspace file sha256_hex'),
      provenance: expectNullableRecord(object.provenance, 'file workspace file provenance'),
      content_path: expectString(object.content_path, 'file workspace file content_path'),
      created_at: expectString(object.created_at, 'file workspace file created_at'),
      updated_at: expectString(object.updated_at, 'file workspace file updated_at'),
    };
  }

  static toSessionFileBindingList(payload: unknown): SessionFileBindingListResponse {
    const object = expectRecord(payload, 'session file binding list response');
    const bindings = object.bindings;
    if (!Array.isArray(bindings)) {
      throw new Error('session file binding list response must contain a bindings array');
    }
    return {
      bindings: bindings.map((binding) => this.toSessionFileBinding(binding)),
    };
  }

  static toSessionFileBinding(payload: unknown): SessionFileBindingResource {
    const object = expectRecord(payload, 'session file binding resource');
    const mediaType = optionalString(object.media_type, 'session file binding media_type');
    const error = optionalString(object.error, 'session file binding error');
    return {
      id: expectString(object.id, 'session file binding id'),
      session_id: expectString(object.session_id, 'session file binding session_id'),
      workspace_id: expectString(object.workspace_id, 'session file binding workspace_id'),
      file_id: expectString(object.file_id, 'session file binding file_id'),
      file_name: expectString(object.file_name, 'session file binding file_name'),
      ...(mediaType !== undefined ? { media_type: mediaType } : {}),
      byte_count: expectNumber(object.byte_count, 'session file binding byte_count'),
      sha256_hex: expectString(object.sha256_hex, 'session file binding sha256_hex'),
      provenance: expectNullableRecord(object.provenance, 'session file binding provenance'),
      mount_path: expectString(object.mount_path, 'session file binding mount_path'),
      mode: expectSessionFileBindingMode(object.mode),
      state: expectSessionFileBindingState(object.state),
      ...(error !== undefined ? { error } : {}),
      labels: expectStringRecord(object.labels, 'session file binding labels'),
      content_path: expectString(object.content_path, 'session file binding content_path'),
      created_at: expectString(object.created_at, 'session file binding created_at'),
      updated_at: expectString(object.updated_at, 'session file binding updated_at'),
    };
  }
}

function expectNullableRecord(
  value: unknown,
  label: string,
): Readonly<Record<string, unknown>> | null {
  if (value === null) {
    return null;
  }
  return expectRecord(value, label);
}

function expectSessionFileBindingMode(value: unknown): SessionFileBindingMode {
  const mode = expectString(value, 'session file binding mode');
  if (!SESSION_FILE_BINDING_MODES.has(mode)) {
    throw new Error('session file binding mode must be a known binding mode');
  }
  return mode as SessionFileBindingMode;
}

function expectSessionFileBindingState(value: unknown): SessionFileBindingState {
  const state = expectString(value, 'session file binding state');
  if (!SESSION_FILE_BINDING_STATES.has(state)) {
    throw new Error('session file binding state must be a known binding state');
  }
  return state as SessionFileBindingState;
}
