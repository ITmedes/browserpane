import { afterEach, describe, expect, it, vi } from 'vitest';

import { createProjectEditDraft } from '$lib/projects/project-edit-view-model';
import type { ProjectResource } from '$lib/projects/project-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ProjectQuotaEditor from './ProjectQuotaEditor.svelte';

afterEach(cleanupRenderedComponents);

describe('ProjectQuotaEditor', () => {
  it('emits draft updates for quota enablement and values', () => {
    const draft = createProjectEditDraft(project());
    const onDraftChange = vi.fn();
    const target = renderComponent(ProjectQuotaEditor, {
      draft,
      onDraftChange,
    });

    setInputValue(byTestId(target, 'project-quota-max-active-sessions-value'), '6');
    expect(onDraftChange).toHaveBeenLastCalledWith(expect.objectContaining({
      maxActiveSessions: { enabled: true, value: '6' },
    }));

    setCheckbox(byTestId(target, 'project-quota-max-egress-total-bytes-enabled'), true);
    expect(onDraftChange).toHaveBeenLastCalledWith(expect.objectContaining({
      maxEgressTotalBytes: { enabled: true, value: '' },
    }));
  });

  it('uses one control for the rolling session creation quota pair', () => {
    const draft = createProjectEditDraft(project());
    const onDraftChange = vi.fn();
    const target = renderComponent(ProjectQuotaEditor, {
      draft,
      onDraftChange,
    });

    expect(target.querySelector('[data-testid="project-quota-max-session-creations-per-window-enabled"]')).toBeNull();
    expect(target.querySelector('[data-testid="project-quota-session-creation-window-sec-enabled"]')).toBeNull();

    setCheckbox(byTestId(target, 'project-quota-session-creation-rate-enabled'), true);
    expect(onDraftChange).toHaveBeenLastCalledWith(expect.objectContaining({
      maxSessionCreationsPerWindow: { enabled: true, value: '' },
      sessionCreationWindowSec: { enabled: true, value: '' },
    }));

    expect(byTestId(target, 'project-quota-max-session-creations-per-window-value')).toBeTruthy();
    expect(byTestId(target, 'project-quota-session-creation-window-sec-value')).toBeTruthy();
  });

  it('renders quota errors inside the affected quota card', () => {
    const draft = createProjectEditDraft(project());
    const target = renderComponent(ProjectQuotaEditor, {
      draft,
      fieldErrors: {
        maxActiveSessions: ['Active sessions quota needs a positive integer.'],
      },
    });

    expect(byTestId(target, 'project-quota-max-active-sessions-error').textContent).toContain(
      'Active sessions quota needs a positive integer.',
    );
  });

  it('renders rolling quota pair errors once inside the combined card', () => {
    const draft = {
      ...createProjectEditDraft(project()),
      maxSessionCreationsPerWindow: { enabled: true, value: '3' },
      sessionCreationWindowSec: { enabled: false, value: '' },
    };
    const target = renderComponent(ProjectQuotaEditor, {
      draft,
      fieldErrors: {
        maxSessionCreationsPerWindow: ['Rolling session creation quota needs both a session limit and a window duration.'],
        sessionCreationWindowSec: ['Rolling session creation quota needs both a session limit and a window duration.'],
      },
    });

    const error = byTestId(target, 'project-quota-session-creation-rate-error');
    expect(error.textContent).toContain('Rolling session creation quota needs both a session limit and a window duration.');
    expect(error.querySelectorAll('li')).toHaveLength(1);
  });
});

function setCheckbox(element: Element, checked: boolean): void {
  (element as HTMLInputElement).checked = checked;
  element.dispatchEvent(new Event('change', { bubbles: true }));
}

function setInputValue(element: Element, value: string): void {
  (element as HTMLInputElement).value = value;
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
