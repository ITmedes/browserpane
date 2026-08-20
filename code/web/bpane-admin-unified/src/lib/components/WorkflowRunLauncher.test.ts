import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkflowRunResource } from '$lib/workflow-runs/workflow-run-types';
import type { WorkflowDefinitionVersionResource } from '$lib/workflows/workflow-types';
import { projectResourceFixture } from '$lib/test-utils/project-fixture';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowRunLauncher from './WorkflowRunLauncher.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowRunLauncher', () => {
  it('renders schema fields, previews payload, and starts a run', async () => {
    const onStartRun = vi.fn(async () => ({
      run: workflowRun(),
      previewOpened: false,
    }));
    const target = renderComponent(WorkflowRunLauncher, {
      workflowId: 'workflow-1',
      selectedVersion: version(),
      onStartRun,
    });

    expect(byTestId(target, 'workflow-run-input-target_url')).toBeInstanceOf(HTMLInputElement);
    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-payload-preview').textContent).toContain('"target_url": "https://browserpane.io/"');
    });

    const stepInput = byTestId(target, 'workflow-run-input-scroll_step_px') as HTMLInputElement;
    stepInput.value = '240';
    stepInput.dispatchEvent(new InputEvent('input', { bubbles: true }));

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-payload-preview').textContent).toContain('"scroll_step_px": 240');
    });
    byTestId(target, 'workflow-run-start').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-launch-success').textContent).toContain('run-1');
    });
    expect(onStartRun).toHaveBeenCalledWith(
      expect.objectContaining({
        workflow_id: 'workflow-1',
        version: 'v1',
        session: { create_session: { labels: { origin: 'admin-unified-workflow-run' } } },
        input: expect.objectContaining({ scroll_step_px: 240 }),
      }),
      { connectPreview: false },
    );
  });

  it('uses default-session mode and delegates start-and-connect', async () => {
    const onStartRun = vi.fn(async () => ({
      run: workflowRun(),
      previewOpened: true,
    }));
    const target = renderComponent(WorkflowRunLauncher, {
      workflowId: 'workflow-1',
      selectedVersion: version({ default_session: { labels: { origin: 'workflow-default' } } }),
      onStartRun,
    });

    await vi.waitFor(() => {
      expect((byTestId(target, 'workflow-run-session-mode') as HTMLSelectElement).value).toBe('version_default');
    });
    byTestId(target, 'workflow-run-start-connect').click();

    await vi.waitFor(() => {
      expect(onStartRun).toHaveBeenCalledWith(expect.not.objectContaining({ session: expect.anything() }), {
        connectPreview: true,
      });
    });
  });

  it('shows validation errors before submitting', async () => {
    const onStartRun = vi.fn();
    const target = renderComponent(WorkflowRunLauncher, {
      workflowId: 'workflow-1',
      selectedVersion: version(),
      onStartRun,
    });
    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-payload-preview').textContent).toContain('"scroll_delay_ms": 100');
    });
    const inputJson = byTestId(target, 'workflow-run-input-json') as HTMLTextAreaElement;
    inputJson.value = '{}';
    inputJson.dispatchEvent(new InputEvent('input', { bubbles: true }));

    byTestId(target, 'workflow-run-start').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-launch-error').textContent).toContain('scroll_delay_ms');
    });
    expect(onStartRun).not.toHaveBeenCalled();
  });

  it('selects active projects, exposes capacity, and blocks archived projects', async () => {
    const onStartRun = vi.fn(async () => ({ run: workflowRun(), previewOpened: false }));
    const activeProject = projectResourceFixture({
      id: 'project-active',
      name: 'Active support',
      quotas: { max_active_workflow_runs: 2 },
      usage: {
        active_workflow_runs: 2,
        max_active_workflow_runs: 2,
        alerts: [{
          metric: 'runtime_usage_ms',
          state: 'approaching_limit',
          current_value: 80,
          limit_value: 100,
          threshold_percent: 80,
          message: 'Runtime budget is approaching its limit.',
        }],
      },
    });
    const archivedProject = projectResourceFixture({
      id: 'project-archived',
      name: 'Archived support',
      state: 'archived',
    });
    const target = renderComponent(WorkflowRunLauncher, {
      workflowId: 'workflow-1',
      selectedVersion: version(),
      projectOptionsState: { status: 'ready', projects: [activeProject, archivedProject] },
      onStartRun,
    });
    await vi.waitFor(() => {
      expect((byTestId(target, 'workflow-run-project-id') as HTMLSelectElement).options.length).toBe(3);
    });

    const projectSelect = byTestId(target, 'workflow-run-project-id') as HTMLSelectElement;
    expect([...projectSelect.options].find((option) => option.value === archivedProject.id)?.disabled).toBe(true);
    setSelectValue(projectSelect, activeProject.id);

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-project-governance').textContent).toContain('Active runs: 2 / 2');
      expect(byTestId(target, 'workflow-run-project-alerts').textContent).toContain('Runtime budget is approaching');
    });
    byTestId(target, 'workflow-run-start').click();

    await vi.waitFor(() => {
      expect(onStartRun).toHaveBeenCalledWith(expect.objectContaining({
        project_id: activeProject.id,
        session: {
          create_session: {
            project_id: activeProject.id,
            labels: { origin: 'admin-unified-workflow-run' },
          },
        },
      }), { connectPreview: false });
    });
  });

  it('keeps owner-scoped launch available when the project catalog fails', async () => {
    const onStartRun = vi.fn(async () => ({ run: workflowRun(), previewOpened: false }));
    const target = renderComponent(WorkflowRunLauncher, {
      workflowId: 'workflow-1',
      selectedVersion: version(),
      projectOptionsState: { status: 'error', message: 'Project API returned HTTP 503.' },
      onStartRun,
    });

    expect(byTestId(target, 'workflow-run-project-options-error').textContent)
      .toContain('Owner-scoped launch remains available');
    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-run-payload-preview').textContent)
        .toContain('"scroll_delay_ms": 100');
    });
    byTestId(target, 'workflow-run-start').click();

    await vi.waitFor(() => {
      expect(onStartRun).toHaveBeenCalledWith(
        expect.not.objectContaining({ project_id: expect.anything() }),
        { connectPreview: false },
      );
    });
  });
});

function setSelectValue(element: HTMLSelectElement, value: string): void {
  element.value = value;
  element.dispatchEvent(new Event('change', { bubbles: true }));
}

function version(overrides: Partial<WorkflowDefinitionVersionResource> = {}): WorkflowDefinitionVersionResource {
  return {
    id: 'version-1',
    workflow_definition_id: 'workflow-1',
    version: 'v1',
    executor: 'playwright',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    source: null,
    input_schema: {
      type: 'object',
      required: ['scroll_delay_ms'],
      properties: {
        target_url: {
          type: 'string',
          default: 'https://browserpane.io/',
        },
        scroll_delay_ms: {
          type: 'number',
          default: 100,
        },
        scroll_step_px: {
          type: 'number',
          default: 120,
        },
      },
    },
    output_schema: null,
    default_session: null,
    allowed_credential_binding_ids: [],
    allowed_extension_ids: [],
    allowed_file_workspace_ids: [],
    created_at: '2026-06-21T09:30:00.000Z',
    ...overrides,
    compatibility: overrides.compatibility ?? { state: 'legacy', warnings: [] },
  };
}

function workflowRun(): WorkflowRunResource {
  return {
    id: 'run-1',
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'version-1',
    workflow_version: 'v1',
    project_id: null,
    project: null,
    source_system: 'admin_unified',
    source_reference: 'workflow-detail',
    client_request_id: null,
    state: 'pending',
    session_id: 'session-1',
    automation_task_id: 'task-1',
    input: {},
    output: null,
    error: null,
    artifact_refs: [],
    source_snapshot: null,
    extensions: [],
    credential_bindings: [],
    workspace_inputs: [],
    produced_files: [],
    recordings: [],
    retention: { logs_expire_at: null, output_expire_at: null },
    project_admission: null,
    admission: null,
    intervention: {},
    runtime: null,
    labels: {},
    started_at: null,
    completed_at: null,
    events_path: '/api/v1/workflow-runs/run-1/events',
    logs_path: '/api/v1/workflow-runs/run-1/logs',
    created_at: '2026-06-21T10:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}
