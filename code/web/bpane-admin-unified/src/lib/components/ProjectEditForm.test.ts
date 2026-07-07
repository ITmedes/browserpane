import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ProjectPolicyOptions, ProjectResource } from '$lib/projects/project-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ProjectEditForm from './ProjectEditForm.svelte';

afterEach(cleanupRenderedComponents);

describe('ProjectEditForm', () => {
  it('creates a fully configured project from every settings group', async () => {
    const onSave = vi.fn();
    const target = renderComponent(ProjectEditForm, {
      mode: 'create',
      policyOptionsState: { status: 'ready', options: options() },
      onSave,
    });

    await inputValue(target, 'project-edit-name', 'Customer Support');
    await selectValue(target, 'project-edit-state', 'archived');
    await inputValue(target, 'project-edit-description', 'Handles escalated customer support sessions');
    await inputValue(target, 'project-edit-labels', 'env=prod\nteam=support');
    await checkbox(target, 'project-policy-browser-uploads', false);
    await checkbox(target, 'project-policy-browser-downloads', false);
    await checkbox(target, 'project-policy-session-file-bindings', false);
    await checkbox(target, 'project-policy-manual-recordings', false);
    await selectValue(target, 'project-policy-budget-enforcement', 'block_session_creation');

    await restrictAndSelect(target, 'session-templates', 'template-support');
    await restrictAndSelect(target, 'browser-contexts', '22222222-2222-4222-8222-222222222222');
    await restrictAndSelect(target, 'egress-profiles', '33333333-3333-4333-8333-333333333333');
    await restrictAndSelect(target, 'extensions', '44444444-4444-4444-8444-444444444444');
    await restrictAndSelect(target, 'file-workspaces', '55555555-5555-4555-8555-555555555555');

    await checkbox(target, 'project-quota-max-active-sessions-enabled', true);
    await inputValue(target, 'project-quota-max-active-sessions-value', '3');
    await checkbox(target, 'project-quota-max-active-workflow-runs-enabled', true);
    await inputValue(target, 'project-quota-max-active-workflow-runs-value', '4');
    await checkbox(target, 'project-quota-max-retained-storage-bytes-enabled', true);
    await inputValue(target, 'project-quota-max-retained-storage-bytes-value', '1073741824');
    await checkbox(target, 'project-quota-max-session-creations-enabled', true);
    await inputValue(target, 'project-quota-max-session-creations-value', '100');
    await checkbox(target, 'project-quota-session-creation-rate-enabled', true);
    await inputValue(target, 'project-quota-max-session-creations-per-window-value', '20');
    await inputValue(target, 'project-quota-session-creation-window-sec-value', '3600');
    await inputValue(target, 'project-quota-max-runtime-usage-ms-slider', '1');
    await checkbox(target, 'project-quota-max-egress-total-bytes-enabled', true);
    await inputValue(target, 'project-quota-max-egress-total-bytes-value', '2147483648');

    byTestId(target, 'project-edit-save').click();

    expect(onSave).toHaveBeenCalledWith({
      name: 'Customer Support',
      description: 'Handles escalated customer support sessions',
      labels: { env: 'prod', team: 'support' },
      state: 'archived',
      policy: {
        allowed_session_template_ids: ['template-support'],
        allowed_egress_profile_ids: ['33333333-3333-4333-8333-333333333333'],
        allowed_extension_ids: ['44444444-4444-4444-8444-444444444444'],
        allowed_browser_context_ids: ['22222222-2222-4222-8222-222222222222'],
        allowed_file_workspace_ids: ['55555555-5555-4555-8555-555555555555'],
        allow_browser_uploads: false,
        allow_browser_downloads: false,
        allow_session_file_bindings: false,
        allow_manual_recordings: false,
        usage_budget_enforcement: 'block_session_creation',
      },
      quotas: {
        max_active_sessions: 3,
        max_active_workflow_runs: 4,
        max_retained_storage_bytes: 1073741824,
        max_session_creations: 100,
        max_session_creations_per_window: 20,
        session_creation_window_sec: 3600,
        max_runtime_usage_ms: 14400000,
        max_egress_total_bytes: 2147483648,
      },
    });
  });

  it('creates a project from the shared settings controls', async () => {
    const onSave = vi.fn();
    const target = renderComponent(ProjectEditForm, {
      mode: 'create',
      onSave,
    });

    expect(byTestId(target, 'project-edit-form').textContent).toContain('New project settings');
    expect(target.querySelector('[data-testid="project-status-summary"]')).toBeNull();
    expect((byTestId(target, 'project-edit-save') as HTMLButtonElement).disabled).toBe(true);

    setInputValue(byTestId(target, 'project-edit-name'), 'Customer Support');
    setInputValue(byTestId(target, 'project-edit-labels'), 'team=support');
    await tick();
    byTestId(target, 'project-edit-save').click();

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      name: 'Customer Support',
      description: null,
      labels: { team: 'support' },
      policy: expect.objectContaining({
        allow_browser_uploads: true,
        usage_budget_enforcement: 'warning_only',
      }),
      state: 'active',
    }));
  });

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
      quotas: expect.objectContaining({ max_active_sessions: 4 }),
      policy: expect.objectContaining({ allow_browser_uploads: true }),
      state: 'active',
    }));
  });

  it('saves edited policy allow-lists and quota limits', async () => {
    const onSave = vi.fn();
    const target = renderComponent(ProjectEditForm, {
      project: project(),
      policyOptionsState: { status: 'ready', options: options() },
      onSave,
    });

    setCheckbox(byTestId(target, 'project-policy-browser-uploads'), false);
    setSelectValue(byTestId(target, 'project-policy-budget-enforcement'), 'block_session_creation');
    setCheckbox(byTestId(target, 'project-policy-session-templates-restrict'), true);
    setCheckbox(byTestId(target, 'project-policy-browser-contexts-restrict'), true);
    await tick();
    setCheckbox(policyOption(target, 'session-templates', 'template-support'), true);
    setCheckbox(policyOption(target, 'browser-contexts', '22222222-2222-4222-8222-222222222222'), true);
    setInputValue(byTestId(target, 'project-quota-max-active-sessions-value'), '6');
    setCheckbox(byTestId(target, 'project-quota-session-creation-rate-enabled'), true);
    setInputValue(byTestId(target, 'project-quota-max-session-creations-per-window-value'), '10');
    setInputValue(byTestId(target, 'project-quota-session-creation-window-sec-value'), '3600');
    await tick();

    byTestId(target, 'project-edit-save').click();

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      quotas: expect.objectContaining({
        max_active_sessions: 6,
        max_session_creations_per_window: 10,
        session_creation_window_sec: 3600,
      }),
      policy: expect.objectContaining({
        allowed_session_template_ids: ['template-support'],
        allowed_browser_context_ids: ['22222222-2222-4222-8222-222222222222'],
        allow_browser_uploads: false,
        usage_budget_enforcement: 'block_session_creation',
      }),
    }));
  });

  it('resets new project drafts back to the empty create state', async () => {
    const target = renderComponent(ProjectEditForm, {
      mode: 'create',
      onSave: vi.fn(),
    });

    await inputValue(target, 'project-edit-name', 'Temporary project');
    expect((byTestId(target, 'project-edit-cancel') as HTMLButtonElement).disabled).toBe(false);

    byTestId(target, 'project-edit-cancel').click();
    await tick();

    expect((byTestId(target, 'project-edit-name') as HTMLInputElement).value).toBe('');
    expect((byTestId(target, 'project-edit-save') as HTMLButtonElement).disabled).toBe(true);
    expect(byTestId(target, 'project-edit-form').textContent).toContain('Project draft');
  });

  it('shows validation errors for invalid input', async () => {
    const target = renderComponent(ProjectEditForm, {
      project: project(),
      onSave: vi.fn(),
    });

    setInputValue(byTestId(target, 'project-edit-name'), ' ');
    setInputValue(byTestId(target, 'project-edit-labels'), 'broken');
    await tick();

    expect(byTestId(target, 'project-edit-name-error').textContent).toContain('Project name is required');
    expect(byTestId(target, 'project-edit-labels-error').textContent).toContain('Label line 1 must use key=value.');
    expect(target.querySelector('[data-testid="project-edit-validation"]')).toBeNull();
    expect((byTestId(target, 'project-edit-save') as HTMLButtonElement).disabled).toBe(true);
  });
});

function setInputValue(element: Element, value: string): void {
  (element as HTMLInputElement | HTMLTextAreaElement).value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
}

function setCheckbox(element: Element, checked: boolean): void {
  (element as HTMLInputElement).checked = checked;
  element.dispatchEvent(new Event('change', { bubbles: true }));
}

function setSelectValue(element: Element, value: string): void {
  (element as HTMLSelectElement).value = value;
  element.dispatchEvent(new Event('change', { bubbles: true }));
}

async function inputValue(target: ParentNode, testId: string, value: string): Promise<void> {
  setInputValue(byTestId(target, testId), value);
  await tick();
}

async function checkbox(target: ParentNode, testId: string, checked: boolean): Promise<void> {
  setCheckbox(byTestId(target, testId), checked);
  await tick();
}

async function selectValue(target: ParentNode, testId: string, value: string): Promise<void> {
  setSelectValue(byTestId(target, testId), value);
  await tick();
}

async function restrictAndSelect(target: ParentNode, group: string, optionId: string): Promise<void> {
  await checkbox(target, `project-policy-${group}-restrict`, true);
  setCheckbox(policyOption(target, group, optionId), true);
  await tick();
}

function policyOption(target: ParentNode, group: string, optionId: string): HTMLElement {
  const option = target.querySelector(`[data-testid="project-policy-${group}-option"][data-option-id="${optionId}"]`);
  expect(option).toBeInstanceOf(HTMLElement);
  return option as HTMLElement;
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
    egressProfiles: [{
      id: '33333333-3333-4333-8333-333333333333',
      name: 'Support Egress',
      description: 'Approved egress profile',
      state: 'active',
      scope: 'owner scoped',
    }],
    extensions: [{
      id: '44444444-4444-4444-8444-444444444444',
      name: 'Support Extension',
      description: 'Approved extension',
      state: 'enabled',
      scope: 'owner scoped',
    }],
    fileWorkspaces: [{
      id: '55555555-5555-4555-8555-555555555555',
      name: 'Support Workspace',
      description: 'Approved file workspace',
      state: 'active',
      scope: 'owner scoped',
    }],
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
