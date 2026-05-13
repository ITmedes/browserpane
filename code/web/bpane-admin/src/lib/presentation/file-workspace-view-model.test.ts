import { describe, expect, it } from 'vitest';
import type {
  FileWorkspaceFileResource,
  FileWorkspaceResource,
  SessionFileBindingResource,
} from '../api/control-types';
import {
  FileWorkspaceViewModelBuilder,
  validateSessionMountPath,
} from './file-workspace-view-model';

describe('FileWorkspaceViewModelBuilder', () => {
  it('filters workspace rows and keeps labels visible', () => {
    const viewModel = FileWorkspaceViewModelBuilder.list({
      workspaces: [WORKSPACE],
      search: 'support',
    });

    expect(viewModel.rows).toHaveLength(1);
    expect(viewModel.rows[0]?.labels).toBe('purpose=support');
    expect(viewModel.emptyMessage).toBe('No file workspaces match the current filter.');
  });

  it('summarizes workspace files and binding metadata', () => {
    const file = FileWorkspaceViewModelBuilder.workspaceFile(WORKSPACE_FILE);
    const binding = FileWorkspaceViewModelBuilder.sessionBinding(BINDING);

    expect(file.digest).toBe('sha256 64ec88ca00b268e5...');
    expect(file.provenance).toContain('source=admin-upload');
    expect(binding.mode).toBe('read only');
    expect(binding.state).toBe('pending');
    expect(binding.error).toBe('No materialization error.');
  });
});

describe('validateSessionMountPath', () => {
  it('accepts normalized relative session paths', () => {
    expect(validateSessionMountPath(' uploads/customer-sample.csv ').value)
      .toBe('uploads/customer-sample.csv');
  });

  it('rejects unsafe or colliding session paths', () => {
    expect(validateSessionMountPath('').message).toBe('Mount path is required.');
    expect(validateSessionMountPath('/tmp/file.csv').message).toBe('Mount path must be relative.');
    expect(validateSessionMountPath('../file.csv').message).toBe('Mount path must not contain traversal components.');
    expect(validateSessionMountPath('inputs//file.csv').message)
      .toBe('Mount path must not contain empty path components.');
    expect(validateSessionMountPath('inputs\\file.csv').message)
      .toBe('Mount path must use forward slashes.');
    expect(validateSessionMountPath('inputs/file.csv', ['inputs/file.csv']).message)
      .toBe('Mount path is already bound for this session.');
  });
});

const WORKSPACE: FileWorkspaceResource = {
  id: 'workspace-1',
  name: 'Admin inputs',
  description: 'Support reproduction files',
  labels: { purpose: 'support' },
  files_path: '/api/v1/file-workspaces/workspace-1/files',
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
};

const WORKSPACE_FILE: FileWorkspaceFileResource = {
  id: 'file-1',
  workspace_id: WORKSPACE.id,
  name: 'customer-sample.csv',
  media_type: 'text/csv',
  byte_count: 22,
  sha256_hex: '64ec88ca00b268e5ba1a35678a1b5316d212f4f366b2477232534a8aeca37f3c',
  provenance: { source: 'admin-upload' },
  content_path: '/api/v1/file-workspaces/workspace-1/files/file-1/content',
  created_at: '2026-05-04T19:02:00Z',
  updated_at: '2026-05-04T19:03:00Z',
};

const BINDING: SessionFileBindingResource = {
  id: 'binding-1',
  session_id: 'session-1',
  workspace_id: WORKSPACE.id,
  file_id: WORKSPACE_FILE.id,
  file_name: WORKSPACE_FILE.name,
  media_type: 'text/csv',
  byte_count: WORKSPACE_FILE.byte_count,
  sha256_hex: WORKSPACE_FILE.sha256_hex,
  provenance: WORKSPACE_FILE.provenance,
  mount_path: 'uploads/customer-sample.csv',
  mode: 'read_only',
  state: 'pending',
  error: null,
  labels: {},
  content_path: '/api/v1/sessions/session-1/file-bindings/binding-1/content',
  created_at: '2026-05-04T19:04:00Z',
  updated_at: '2026-05-04T19:05:00Z',
};
