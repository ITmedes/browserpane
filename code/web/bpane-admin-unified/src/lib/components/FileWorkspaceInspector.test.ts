import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { FileWorkspaceFileResource, FileWorkspaceResource } from '$lib/file-workspaces/file-workspace-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import FileWorkspaceInspector from './FileWorkspaceInspector.svelte';

afterEach(cleanupRenderedComponents);

describe('FileWorkspaceInspector', () => {
  it('renders metadata and delegates file actions', async () => {
    const onRefreshWorkspace = vi.fn();
    const onDownloadFile = vi.fn();
    const onDeleteFile = vi.fn();
    const target = renderComponent(FileWorkspaceInspector, {
      state: { status: 'ready', workspace: workspace(), files: [file()] },
      onRefreshWorkspace,
      onDownloadFile,
      onDeleteFile,
    });

    expect(byTestId(target, 'file-workspace-detail-name').textContent).toContain('Support inputs');
    expect(byTestId(target, 'file-workspace-detail-labels').textContent).toContain('team=support');
    expect(byTestId(target, 'file-workspace-detail-file-count').textContent).toContain('1 file');
    byTestId(target, 'file-workspace-refresh-detail').click();
    byTestId(target, 'file-workspace-file-download').click();
    byTestId(target, 'file-workspace-file-delete').click();

    expect(onRefreshWorkspace).toHaveBeenCalledOnce();
    expect(onDownloadFile).toHaveBeenCalledWith(expect.objectContaining({ id: 'file-1' }));
    expect(onDeleteFile).toHaveBeenCalledWith(expect.objectContaining({ id: 'file-1' }));

    byTestId(target, 'file-workspace-upload-submit').click();
    await tick();
    expect(byTestId(target, 'file-workspace-upload-error').textContent).toContain('Choose a file');
  });

  it('renders idle, loading, and error states', () => {
    let target = renderComponent(FileWorkspaceInspector, { state: { status: 'idle' } });
    expect(byTestId(target, 'file-workspace-inspector-idle').textContent).toContain('Select a file workspace');

    void cleanupRenderedComponents();
    target = renderComponent(FileWorkspaceInspector, { state: { status: 'loading', workspaceId: 'workspace-1' } });
    expect(byTestId(target, 'file-workspace-inspector-loading').textContent).toContain('Loading file workspace');

    void cleanupRenderedComponents();
    target = renderComponent(FileWorkspaceInspector, {
      state: { status: 'error', workspaceId: 'workspace-1', message: 'Missing workspace.' },
    });
    expect(byTestId(target, 'file-workspace-inspector-error').textContent).toContain('Missing workspace');
  });
});

function workspace(): FileWorkspaceResource {
  return {
    id: 'workspace-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    name: 'Support inputs',
    description: 'Reusable files',
    labels: { team: 'support' },
    files_path: '/api/v1/file-workspaces/workspace-1/files',
    created_at: '2026-06-20T09:00:00.000Z',
    updated_at: '2026-06-20T10:00:00.000Z',
  };
}

function file(): FileWorkspaceFileResource {
  return {
    id: 'file-1',
    workspace_id: 'workspace-1',
    name: 'fixture.csv',
    media_type: 'text/csv',
    byte_count: 18,
    sha256_hex: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
    provenance: { source: 'test' },
    content_path: '/api/v1/file-workspaces/workspace-1/files/file-1/content',
    created_at: '2026-06-20T09:30:00.000Z',
    updated_at: '2026-06-20T09:30:00.000Z',
  };
}
