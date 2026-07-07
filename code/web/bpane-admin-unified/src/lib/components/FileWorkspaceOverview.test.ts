import { afterEach, describe, expect, it, vi } from 'vitest';

import type { FileWorkspaceResource } from '$lib/file-workspaces/file-workspace-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import FileWorkspaceOverview from './FileWorkspaceOverview.svelte';

afterEach(cleanupRenderedComponents);

describe('FileWorkspaceOverview', () => {
  it('renders loading, error, empty, and ready states', () => {
    let target = renderComponent(FileWorkspaceOverview, { state: { status: 'loading' } });
    expect(byTestId(target, 'file-workspaces-loading').textContent).toContain('Loading file workspaces');

    void cleanupRenderedComponents();
    target = renderComponent(FileWorkspaceOverview, {
      state: { status: 'error', message: 'Catalog unavailable.' },
    });
    expect(byTestId(target, 'file-workspaces-error').textContent).toContain('Catalog unavailable');

    void cleanupRenderedComponents();
    target = renderComponent(FileWorkspaceOverview, {
      state: { status: 'ready', workspaces: [], fileCounts: {} },
    });
    expect(byTestId(target, 'file-workspaces-empty').textContent).toContain('catalog is empty');

    void cleanupRenderedComponents();
    target = renderComponent(FileWorkspaceOverview, {
      state: {
        status: 'ready',
        workspaces: [workspace()],
        fileCounts: { 'workspace-1': 1 },
      },
    });
    expect(byTestId(target, 'file-workspaces-metric-total').textContent).toContain('1');
    expect(byTestId(target, 'file-workspaces-list').textContent).toContain('Support inputs');
  });

  it('delegates refresh requests', () => {
    const onRefresh = vi.fn();
    const target = renderComponent(FileWorkspaceOverview, {
      state: {
        status: 'ready',
        workspaces: [workspace()],
        fileCounts: { 'workspace-1': 1 },
      },
      onRefresh,
    });

    byTestId(target, 'file-workspaces-refresh-button').click();

    expect(onRefresh).toHaveBeenCalledOnce();
  });
});

function workspace(): FileWorkspaceResource {
  return {
    id: 'workspace-1',
    project_id: null,
    project: null,
    name: 'Support inputs',
    description: null,
    labels: {},
    files_path: '/api/v1/file-workspaces/workspace-1/files',
    created_at: '2026-06-20T09:00:00.000Z',
    updated_at: '2026-06-20T10:00:00.000Z',
  };
}
