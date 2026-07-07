import { afterEach, describe, expect, it, vi } from 'vitest';

import { createProjectEditDraft } from '$lib/projects/project-edit-view-model';
import type { ProjectPolicyOptions, ProjectResource } from '$lib/projects/project-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ProjectPolicyEditor from './ProjectPolicyEditor.svelte';

afterEach(cleanupRenderedComponents);

describe('ProjectPolicyEditor', () => {
  it('emits draft updates for boolean policy controls', () => {
    const draft = createProjectEditDraft(project());
    const onDraftChange = vi.fn();
    const target = renderComponent(ProjectPolicyEditor, {
      draft,
      policyOptionsState: { status: 'ready', options: options() },
      onDraftChange,
    });

    setCheckbox(byTestId(target, 'project-policy-browser-uploads'), false);

    expect(onDraftChange).toHaveBeenCalledWith(expect.objectContaining({
      allowBrowserUploads: false,
    }));
  });

  it('renders restricted selector options and emits selected resource ids', () => {
    const draft = {
      ...createProjectEditDraft(project()),
      restrictSessionTemplates: true,
    };
    const onDraftChange = vi.fn();
    const target = renderComponent(ProjectPolicyEditor, {
      draft,
      policyOptionsState: { status: 'ready', options: options() },
      onDraftChange,
    });

    const option = target.querySelector(
      '[data-testid="project-policy-session-templates-option"][data-option-id="template-support"]',
    );
    expect(option).toBeInstanceOf(HTMLInputElement);
    setCheckbox(option as HTMLInputElement, true);

    expect(onDraftChange).toHaveBeenCalledWith(expect.objectContaining({
      allowedSessionTemplateIds: ['template-support'],
    }));
  });

  it('renders selector errors inside the affected allow-list group', () => {
    const draft = {
      ...createProjectEditDraft(project()),
      restrictSessionTemplates: true,
    };
    const target = renderComponent(ProjectPolicyEditor, {
      draft,
      policyOptionsState: { status: 'ready', options: options() },
      fieldErrors: {
        allowedSessionTemplateIds: ['Session Templates restriction needs at least one selected resource.'],
      },
    });

    expect(byTestId(target, 'project-policy-session-templates-error').textContent).toContain(
      'Session Templates restriction needs at least one selected resource.',
    );
  });
});

function setCheckbox(element: Element, checked: boolean): void {
  (element as HTMLInputElement).checked = checked;
  element.dispatchEvent(new Event('change', { bubbles: true }));
}

function options(): ProjectPolicyOptions {
  return {
    sessionTemplates: [{
      id: 'template-support',
      name: 'Support Browser',
      description: 'Approved template',
      state: 'v1',
      scope: null,
    }],
    browserContexts: [{
      id: '22222222-2222-4222-8222-222222222222',
      name: 'Support Context',
      description: null,
      state: 'ready',
      scope: 'owner scoped',
    }],
    egressProfiles: [],
    extensions: [],
    fileWorkspaces: [],
  };
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
