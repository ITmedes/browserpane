import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ProjectResource } from '$lib/projects/project-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ProjectEditForm from './ProjectEditForm.svelte';

afterEach(cleanupRenderedComponents);

describe('ProjectEditForm', () => {
  it('validates and saves safe project fields while preserving policy and quotas', async () => {
    const onSave = vi.fn();
    const target = renderComponent(ProjectEditForm, {
      project: project(),
      onSave,
    });

    expect((byTestId(target, 'project-edit-save') as HTMLButtonElement).disabled).toBe(true);

    setInputValue(byTestId(target, 'project-edit-description'), 'Updated support project');
    setInputValue(byTestId(target, 'project-edit-labels'), 'env=prod\nteam=support');
    await tick();

    byTestId(target, 'project-edit-save').click();
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      name: 'Support',
      description: 'Updated support project',
      labels: { env: 'prod', team: 'support' },
      quotas: { max_active_sessions: 4 },
      policy: expect.objectContaining({ allow_browser_uploads: true }),
      state: 'active',
    }));
  });

  it('shows validation errors for invalid input', async () => {
    const target = renderComponent(ProjectEditForm, {
      project: project(),
      onSave: vi.fn(),
    });

    setInputValue(byTestId(target, 'project-edit-name'), ' ');
    setInputValue(byTestId(target, 'project-edit-labels'), 'broken');
    await tick();

    expect(byTestId(target, 'project-edit-validation').textContent).toContain('Project name is required');
    expect((byTestId(target, 'project-edit-save') as HTMLButtonElement).disabled).toBe(true);
  });
});

function setInputValue(element: Element, value: string): void {
  (element as HTMLInputElement | HTMLTextAreaElement).value = value;
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
