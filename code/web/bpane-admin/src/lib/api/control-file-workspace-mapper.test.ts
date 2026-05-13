import { describe, expect, it } from 'vitest';
import { ControlFileWorkspaceMapper } from './control-file-workspace-mapper';

const WORKSPACE = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  name: 'Admin inputs',
  description: null,
  labels: { suite: 'admin' },
  files_path: '/api/v1/file-workspaces/019df4d2-f4f7-7b00-9e0c-79683b1c82f6/files',
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
};

const WORKSPACE_FILE = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f7',
  workspace_id: WORKSPACE.id,
  name: 'customer-sample.csv',
  media_type: 'text/csv',
  byte_count: 22,
  sha256_hex: '64ec88ca00b268e5ba1a35678a1b5316d212f4f366b2477232534a8aeca37f3c',
  provenance: { source: 'admin-upload' },
  content_path: `/api/v1/file-workspaces/${WORKSPACE.id}/files/019df4d2-f4f7-7b00-9e0c-79683b1c82f7/content`,
  created_at: '2026-05-04T19:02:00Z',
  updated_at: '2026-05-04T19:03:00Z',
};

const BINDING = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f8',
  session_id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f9',
  workspace_id: WORKSPACE.id,
  file_id: WORKSPACE_FILE.id,
  file_name: WORKSPACE_FILE.name,
  media_type: WORKSPACE_FILE.media_type,
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

describe('ControlFileWorkspaceMapper', () => {
  it('maps file workspace and file lists with explicit nullable metadata', () => {
    const workspaces = ControlFileWorkspaceMapper.toWorkspaceList({ workspaces: [WORKSPACE] });
    const files = ControlFileWorkspaceMapper.toWorkspaceFileList({
      files: [{ ...WORKSPACE_FILE, media_type: null, provenance: null }],
    });

    expect(workspaces.workspaces[0]).toMatchObject({
      id: WORKSPACE.id,
      name: 'Admin inputs',
      description: null,
    });
    expect(files.files[0]?.media_type).toBeNull();
    expect(files.files[0]?.provenance).toBeNull();
  });

  it('maps session file bindings and preserves provenance', () => {
    const response = ControlFileWorkspaceMapper.toSessionFileBindingList({ bindings: [BINDING] });

    expect(response.bindings[0]).toMatchObject({
      file_name: 'customer-sample.csv',
      mount_path: 'uploads/customer-sample.csv',
      mode: 'read_only',
      state: 'pending',
      provenance: { source: 'admin-upload' },
    });
  });

  it('rejects malformed wire shapes with actionable resource labels', () => {
    expect(() => ControlFileWorkspaceMapper.toWorkspaceList({ workspaces: null }))
      .toThrow('file workspace list response must contain a workspaces array');
    expect(() => ControlFileWorkspaceMapper.toWorkspaceFile({ ...WORKSPACE_FILE, provenance: 'bad' }))
      .toThrow('file workspace file provenance must be an object');
    expect(() => ControlFileWorkspaceMapper.toSessionFileBinding({ ...BINDING, mode: 'execute' }))
      .toThrow('session file binding mode must be a known binding mode');
  });
});
