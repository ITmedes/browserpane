import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ProjectResource } from '$lib/projects/project-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ProjectInspector from './ProjectInspector.svelte';

afterEach(cleanupRenderedComponents);

describe('ProjectInspector', () => {
  it('renders idle, error, and ready states', async () => {
    let target = renderComponent(ProjectInspector, { state: { status: 'idle' } });
    expect(byTestId(target, 'project-inspector-idle').textContent).toContain('Select a project');

    await cleanupRenderedComponents();
    target = renderComponent(ProjectInspector, {
      state: {
        status: 'error',
        projectId: 'project-a',
        message: 'not found',
      },
    });
    expect(byTestId(target, 'project-inspector-error').textContent).toContain('not found');

    await cleanupRenderedComponents();
    target = renderComponent(ProjectInspector, {
      state: {
        status: 'ready',
        project: project(),
      },
    });
    expect(byTestId(target, 'project-inspector').textContent).toContain('Support');
    expect(byTestId(target, 'project-readonly-details').textContent).toContain('Quotas');
  });

  it('delegates refresh, usage refresh, and save actions', async () => {
    const onRefreshProject = vi.fn();
    const onRefreshUsage = vi.fn();
    const onSaveProject = vi.fn();
    const target = renderComponent(ProjectInspector, {
      state: {
        status: 'ready',
        project: project(),
      },
      onRefreshProject,
      onRefreshUsage,
      onSaveProject,
    });

    byTestId(target, 'project-refresh-detail').click();
    byTestId(target, 'project-refresh-usage').click();
    setInputValue(byTestId(target, 'project-edit-description'), 'Updated support project');
    await tick();
    byTestId(target, 'project-edit-save').click();

    expect(onRefreshProject).toHaveBeenCalledOnce();
    expect(onRefreshUsage).toHaveBeenCalledOnce();
    expect(onSaveProject).toHaveBeenCalledWith(expect.objectContaining({
      description: 'Updated support project',
    }));
  });
});

function setInputValue(element: Element, value: string): void {
  (element as HTMLTextAreaElement).value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
}

function project(): ProjectResource {
  return {
    id: '11111111-1111-4111-8111-111111111111',
    name: 'Support',
    description: 'Support browser work',
    labels: { team: 'support' },
    quotas: { max_active_sessions: 4 },
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
      project_id: '11111111-1111-4111-8111-111111111111',
      active_sessions: 1,
      queued_sessions: 0,
      session_creations: 1,
      max_session_creations: null,
      max_active_sessions: 4,
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
