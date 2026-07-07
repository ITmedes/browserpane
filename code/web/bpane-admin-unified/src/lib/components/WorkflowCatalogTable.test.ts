import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';

import type {
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
} from '$lib/workflows/workflow-types';
import {
  ADMIN_TEMPLATE_LABEL,
  BROWSERPANE_TOUR_TEMPLATE,
} from '$lib/workflows/workflow-visibility';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowCatalogTable from './WorkflowCatalogTable.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowCatalogTable', () => {
  it('renders workflow rows and filters by lens and search text', async () => {
    const target = renderComponent(WorkflowCatalogTable, {
      definitions: [
        workflow({ id: 'tour', name: 'BrowserPane Tour', template: true }),
        workflow({ id: 'plain', name: 'Customer audit' }),
      ],
      versions: {
        tour: version('tour'),
        plain: null,
      },
      hiddenCount: 1,
    });

    expect(byTestId(target, 'workflows-list-count').textContent).toContain('2 of 2');
    expect(target.querySelectorAll('[data-testid="workflows-list-row"]')).toHaveLength(2);
    expect(byTestId(target, 'workflows-list').textContent).toContain('BrowserPane Tour');
    expect(byTestId(target, 'workflows-list').textContent).toContain('Customer audit');
    expect((target.querySelector('[data-testid="workflows-detail-link"]') as HTMLAnchorElement).href).toBe(
      'http://localhost:3000/admin-new/workflows/tour',
    );

    byTestId(target, 'workflows-lens-templates').click();
    await tick();

    expect(byTestId(target, 'workflows-list-count').textContent).toContain('1 of 2');
    expect(byTestId(target, 'workflows-list').textContent).toContain('BrowserPane Tour');
    expect(byTestId(target, 'workflows-list').textContent).not.toContain('Customer audit');

    byTestId(target, 'workflows-lens-all').click();
    const search = byTestId(target, 'workflows-search') as HTMLInputElement;
    search.value = 'missing';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();

    expect(byTestId(target, 'workflows-filter-empty').textContent).toContain('No workflows match');
  });
});

function workflow(options: {
  readonly id: string;
  readonly name: string;
  readonly template?: boolean;
}): WorkflowDefinitionResource {
  return {
    id: options.id,
    name: options.name,
    description: `${options.name} description`,
    labels: options.template ? { [ADMIN_TEMPLATE_LABEL]: BROWSERPANE_TOUR_TEMPLATE } : { team: 'support' },
    latest_version: options.template ? 'v1' : null,
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}

function version(workflowId: string): WorkflowDefinitionVersionResource {
  return {
    id: `${workflowId}-version-1`,
    workflow_definition_id: workflowId,
    version: 'v1',
    executor: 'playwright',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
      root_path: 'dev',
    },
    allowed_credential_binding_ids: [],
    allowed_extension_ids: [],
    allowed_file_workspace_ids: [],
    created_at: '2026-06-21T09:30:00.000Z',
  };
}
