import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ProjectPolicyOption, ProjectResource } from '$lib/projects/project-types';
import type { CreateSessionRequest } from '$lib/sessions/session-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionCreateForm from './SessionCreateForm.svelte';

afterEach(async () => {
  await cleanupRenderedComponents();
});

describe('SessionCreateForm', () => {
  it('submits an empty request when no metadata is configured', async () => {
    const onSave = vi.fn<(request: CreateSessionRequest) => void>();
    const target = renderComponent(SessionCreateForm, {
      optionsState: { status: 'ready', options: emptyOptions() },
      onSave,
    });

    expect(byTestId(target, 'session-create-payload').textContent?.trim()).toBe('{}');
    byTestId(target, 'session-create-save').click();

    expect(onSave).toHaveBeenCalledWith({});
  });

  it('submits explicit capability overrides from the capability checkboxes', async () => {
    const onSave = vi.fn<(request: CreateSessionRequest) => void>();
    const target = renderComponent(SessionCreateForm, {
      optionsState: { status: 'ready', options: emptyOptions() },
      onSave,
    });

    setCheckboxChecked(byTestId(target, 'session-create-capability-microphone'), false);
    setCheckboxChecked(byTestId(target, 'session-create-capability-camera'), false);
    await tick();
    expect(byTestId(target, 'session-create-payload').textContent).toContain('"camera": false');

    byTestId(target, 'session-create-save').click();

    expect(onSave).toHaveBeenCalledWith({
      capabilities: {
        browser_input: true,
        clipboard: true,
        audio: true,
        microphone: false,
        camera: false,
        file_transfer: true,
        resize: true,
      },
    });
  });

  it('submits only explicitly selected session metadata', async () => {
    const onSave = vi.fn<(request: CreateSessionRequest) => void>();
    const target = renderComponent(SessionCreateForm, {
      optionsState: {
        status: 'ready',
        options: {
          projects: [project({ id: 'project-1' })],
          sessionTemplates: [option({ id: 'template-1', name: 'Support Template' })],
          browserContexts: [option({ id: 'context-1', name: 'Reusable Context' })],
          egressProfiles: [option({ id: 'egress-1', name: 'Proxy Egress' })],
        },
      },
      onSave,
    });

    setSelectValue(byTestId(target, 'session-create-project-id'), 'project-1');
    setSelectValue(byTestId(target, 'session-create-template-id'), 'template-1');
    setSelectValue(byTestId(target, 'session-create-owner-mode'), 'exclusive_browser_owner');
    setSelectValue(byTestId(target, 'session-create-browser-context-mode'), 'reusable');
    setSelectValue(byTestId(target, 'session-create-browser-context-id'), 'context-1');
    setSelectValue(byTestId(target, 'session-create-egress-profile-id'), 'egress-1');
    setInputValue(byTestId(target, 'session-create-labels'), 'team=support');
    await tick();
    byTestId(target, 'session-create-save').click();

    expect(onSave).toHaveBeenCalledWith({
      project_id: 'project-1',
      template_id: 'template-1',
      owner_mode: 'exclusive_browser_owner',
      browser_context: { mode: 'reusable', context_id: 'context-1' },
      labels: { team: 'support' },
      network_identity: { egress_profile_id: 'egress-1' },
    });
  });

  it('submits always-on recording only after the recording checkbox is enabled', async () => {
    const onSave = vi.fn<(request: CreateSessionRequest) => void>();
    const target = renderComponent(SessionCreateForm, {
      optionsState: { status: 'ready', options: emptyOptions() },
      onSave,
    });

    setCheckboxChecked(byTestId(target, 'session-create-recording-enabled'), true);
    setInputValue(byTestId(target, 'session-create-recording-retention'), '3600');
    await tick();

    expect(byTestId(target, 'session-create-payload').textContent).toContain('"mode": "always"');
    byTestId(target, 'session-create-save').click();

    expect(onSave).toHaveBeenCalledWith({
      recording: {
        mode: 'always',
        format: 'webm',
        retention_sec: 3600,
      },
    });
  });
});

function setInputValue(element: Element, value: string): void {
  (element as HTMLInputElement | HTMLTextAreaElement).value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
}

function setSelectValue(element: Element, value: string): void {
  (element as HTMLSelectElement).value = value;
  element.dispatchEvent(new Event('change', { bubbles: true }));
}

function setCheckboxChecked(element: Element, checked: boolean): void {
  (element as HTMLInputElement).checked = checked;
  element.dispatchEvent(new Event('change', { bubbles: true }));
}

function emptyOptions() {
  return {
    projects: [],
    sessionTemplates: [],
    browserContexts: [],
    egressProfiles: [],
  };
}

function option(overrides: Partial<ProjectPolicyOption> = {}): ProjectPolicyOption {
  return {
    id: 'option-1',
    projectId: null,
    name: 'Option',
    description: null,
    state: 'ready',
    scope: 'owner scoped',
    ...overrides,
  };
}

function project(overrides: Partial<ProjectResource> = {}): ProjectResource {
  const id = overrides.id ?? 'project-1';
  return {
    id,
    name: 'Project',
    description: null,
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
      project_id: id,
      active_sessions: 0,
      queued_sessions: 0,
      session_creations: 0,
      max_session_creations: null,
      max_active_sessions: null,
      active_workflow_runs: 0,
      max_active_workflow_runs: null,
      runtime_usage_ms: 0,
      max_runtime_usage_ms: null,
      egress_rx_bytes: 0,
      egress_tx_bytes: 0,
      egress_total_bytes: 0,
      max_egress_total_bytes: null,
      retained_storage_bytes: 0,
      max_retained_storage_bytes: null,
      alerts: [],
      observed_at: '2026-06-29T12:00:00Z',
    },
    created_at: '2026-06-29T12:00:00Z',
    updated_at: '2026-06-29T12:00:00Z',
    ...overrides,
  };
}
