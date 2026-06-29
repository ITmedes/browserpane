import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';

import type { FileWorkspaceResource } from '$lib/file-workspaces/file-workspace-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import FileWorkspaceCatalogTable from './FileWorkspaceCatalogTable.svelte';

afterEach(cleanupRenderedComponents);

describe('FileWorkspaceCatalogTable', () => {
  it('renders workspace rows and filters by lens and search text', async () => {
    const target = renderComponent(FileWorkspaceCatalogTable, {
      workspaces: [
        workspace({ id: 'owner-workspace', name: 'Owner inputs', project: false }),
        workspace({ id: 'project-workspace', name: 'Project inputs', project: true }),
      ],
      fileCounts: {
        'owner-workspace': 2,
        'project-workspace': null,
      },
    });

    expect(byTestId(target, 'file-workspaces-list-count').textContent).toContain('2 of 2');
    expect(target.querySelectorAll('[data-testid="file-workspaces-list-row"]')).toHaveLength(2);
    expect(byTestId(target, 'file-workspaces-list').textContent).toContain('Owner inputs');
    expect(byTestId(target, 'file-workspaces-list').textContent).toContain('Project inputs');
    expect((target.querySelector('[data-testid="file-workspaces-detail-link"]') as HTMLAnchorElement).href).toBe(
      'http://localhost:3000/admin-new/files/workspaces/owner-workspace',
    );

    byTestId(target, 'file-workspaces-lens-attention').click();
    await tick();

    expect(byTestId(target, 'file-workspaces-list-count').textContent).toContain('1 of 2');
    expect(byTestId(target, 'file-workspaces-list').textContent).toContain('Project inputs');
    expect(byTestId(target, 'file-workspaces-list').textContent).not.toContain('Owner inputs');

    byTestId(target, 'file-workspaces-lens-all').click();
    const search = byTestId(target, 'file-workspaces-search') as HTMLInputElement;
    search.value = 'missing';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();

    expect(byTestId(target, 'file-workspaces-filter-empty').textContent).toContain('No file workspaces match');
  });
});

function workspace(options: {
  readonly id: string;
  readonly name: string;
  readonly project: boolean;
}): FileWorkspaceResource {
  return {
    id: options.id,
    project_id: options.project ? 'project-1' : null,
    project: options.project ? { id: 'project-1', name: 'Support', state: 'active' } : null,
    name: options.name,
    description: `${options.name} reusable files`,
    labels: { team: options.project ? 'support' : 'platform' },
    files_path: `/api/v1/file-workspaces/${options.id}/files`,
    created_at: '2026-06-20T09:00:00.000Z',
    updated_at: '2026-06-20T10:00:00.000Z',
  };
}
