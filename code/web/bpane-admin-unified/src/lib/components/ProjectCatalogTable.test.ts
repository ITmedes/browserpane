import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';

import type { ProjectResource } from '$lib/projects/project-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ProjectCatalogTable from './ProjectCatalogTable.svelte';

afterEach(cleanupRenderedComponents);

describe('ProjectCatalogTable', () => {
  it('renders rows, filters locally, and links to project details', async () => {
    const target = renderComponent(ProjectCatalogTable, {
      projects: [
        project({ id: '11111111-1111-4111-8111-111111111111', name: 'Support' }),
        project({ id: '22222222-2222-4222-8222-222222222222', name: 'Archive', state: 'archived' }),
      ],
    });

    expect(byTestId(target, 'projects-list-count').textContent).toContain('2 of 2');
    expect(target.querySelectorAll('[data-testid="projects-list-row"]')).toHaveLength(2);
    expect((target.querySelector('[data-testid="projects-detail-link"]') as HTMLAnchorElement).href).toBe(
      'http://localhost:3000/admin-new/projects/11111111-1111-4111-8111-111111111111',
    );

    const search = byTestId(target, 'projects-search') as HTMLInputElement;
    search.value = 'missing';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();

    expect(byTestId(target, 'projects-filter-empty').textContent).toContain('No projects match');
  });
});

function project(options: {
  readonly id: string;
  readonly name: string;
  readonly state?: ProjectResource['state'];
}): ProjectResource {
  return {
    id: options.id,
    name: options.name,
    description: `${options.name} project`,
    labels: {},
    quotas: {},
    policy: {
      allowed_session_template_ids: [],
      allowed_egress_profile_ids: [],
      allowed_extension_ids: [],
      allowed_browser_context_ids: [],
      allowed_file_workspace_ids: [],
      allow_browser_uploads: true,
      allow_browser_downloads: true,
      allow_session_file_bindings: true,
      allow_manual_recordings: true,
      usage_budget_enforcement: 'warning_only',
    },
    state: options.state ?? 'active',
    usage: {
      project_id: options.id,
      active_sessions: 1,
      queued_sessions: 0,
      session_creations: 1,
      max_session_creations: null,
      max_active_sessions: null,
      active_workflow_runs: 0,
      max_active_workflow_runs: null,
      runtime_usage_ms: 30_000,
      max_runtime_usage_ms: null,
      egress_rx_bytes: 0,
      egress_tx_bytes: 0,
      egress_total_bytes: 0,
      max_egress_total_bytes: null,
      retained_storage_bytes: 0,
      max_retained_storage_bytes: null,
      alerts: [],
      observed_at: '2026-06-11T10:00:00.000Z',
    },
    created_at: '2026-06-11T09:00:00.000Z',
    updated_at: '2026-06-11T10:00:00.000Z',
  };
}
