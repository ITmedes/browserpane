import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ProjectResource } from '$lib/projects/project-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ProjectOverview from './ProjectOverview.svelte';

afterEach(cleanupRenderedComponents);

describe('ProjectOverview', () => {
  it('renders loading, error, and empty states', async () => {
    let target = renderComponent(ProjectOverview, { state: { status: 'loading' } });
    expect(byTestId(target, 'projects-loading').textContent).toContain('Loading projects');

    await cleanupRenderedComponents();
    target = renderComponent(ProjectOverview, {
      state: { status: 'error', message: 'No active admin access token is available.' },
    });
    expect(byTestId(target, 'projects-error').textContent).toContain('Project catalog unavailable');

    await cleanupRenderedComponents();
    target = renderComponent(ProjectOverview, { state: { status: 'ready', projects: [] } });
    expect(byTestId(target, 'projects-empty').textContent).toContain('Project catalog is empty');
  });

  it('renders project metrics, list, selected detail, and refresh action', () => {
    const onRefresh = vi.fn();
    const target = renderComponent(ProjectOverview, {
      state: {
        status: 'ready',
        projects: [project()],
        selectedProjectId: 'project-a',
      },
      onRefresh,
    });

    expect(byTestId(target, 'projects-metric-total').textContent).toContain('1');
    expect(byTestId(target, 'projects-list').textContent).toContain('Support');
    expect(byTestId(target, 'projects-selected-detail').textContent).toContain('Budget enforcement');

    byTestId(target, 'projects-refresh-button').click();
    expect(onRefresh).toHaveBeenCalledOnce();
  });
});

function project(): ProjectResource {
  return {
    id: 'project-a',
    name: 'Support',
    description: 'Support project',
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
    state: 'active',
    usage: {
      project_id: 'project-a',
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
