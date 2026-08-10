import { describe, expect, it } from 'vitest';
import {
  buildWorkflowRunDetailModel,
  formatWorkflowRunBytes,
  formatWorkflowRunJson,
  workflowRunControlAvailability,
  workflowRunDefinitionHref,
  workflowRunDetailHref,
  workflowRunIdFromPathname,
  workflowRunProjectHref,
  workflowRunSessionHref,
  workflowRunSessionPreviewHref,
} from './workflow-run-detail-view-model';
import type { WorkflowRunResource } from './workflow-run-types';

describe('workflow run detail view model', () => {
  it('builds operational facts and related resource labels', () => {
    const model = buildWorkflowRunDetailModel(workflowRun());

    expect(model.projectLabel).toBe('Support');
    expect(model.sourceLabel).toBe('external_scheduler / ticket-42');
    expect(model.runtimeLabel).toContain('live_runtime');
    expect(model.admissionLabel).toBe('allowed: policy_ok');
    expect(model.projectHref).toBe('/admin-new/projects/project-1');
    expect(model.admissionEvidence).toMatchObject({
      state: 'allowed',
      reasonCode: 'policy_ok',
      message: 'Workflow run allowed.',
      checkedAt: expect.stringContaining('2026'),
    });
    expect(model.admissionEvidence.facts).toContainEqual(expect.objectContaining({
      testId: 'workflow-run-admission-active-workflows',
      value: '1 / 4',
    }));
    expect(model.facts.map((fact) => fact.testId)).toContain('workflow-run-detail-state');
    expect(model.stateTone).toBe('warning');
  });

  it('keeps generic queue evidence alongside the project admission snapshot', () => {
    const model = buildWorkflowRunDetailModel(workflowRun({
      state: 'queued',
      projectAdmission: {
        state: 'queued',
        reason_code: 'workflow_capacity_reached',
        message: 'Workflow run queued until project capacity is available.',
        active_workflow_runs: 4,
        max_active_workflow_runs: 4,
        checked_at: '2026-08-07T10:00:00.000Z',
      },
      admission: {
        state: 'queued',
        reason: 'project_workflow_concurrency_limit',
        queued_at: '2026-08-07T10:00:01.000Z',
      },
    }));

    expect(model.admissionEvidence).toMatchObject({
      state: 'queued',
      tone: 'warning',
      queueReason: 'project_workflow_concurrency_limit',
      queuedAt: expect.stringContaining('2026'),
    });
  });

  it.each([
    ['pending', true, false],
    ['queued', true, false],
    ['running', true, false],
    ['awaiting_input', true, true],
    ['cancelling', false, false],
    ['succeeded', false, false],
    ['failed', false, false],
    ['cancelled', false, false],
    ['timed_out', false, false],
    ['unknown', false, false],
  ])('gates controls for %s', (state, canCancel, canResolveIntervention) => {
    expect(workflowRunControlAvailability(workflowRun({ state }))).toMatchObject({
      canCancel,
      canResolveIntervention,
    });
  });

  it('requires both awaiting state and a pending intervention', () => {
    expect(workflowRunControlAvailability(workflowRun({ pendingRequest: false })))
      .toMatchObject({ canResolveIntervention: false });
    expect(workflowRunControlAvailability(workflowRun({ state: 'running' })))
      .toMatchObject({ canResolveIntervention: false });
  });

  it('parses canonical and compatibility detail routes safely', () => {
    expect(workflowRunIdFromPathname('/admin-new/workflow-runs/run%2F1')).toBe('run/1');
    expect(workflowRunIdFromPathname('/admin-new/runs/run-1/')).toBe('run-1');
    expect(workflowRunIdFromPathname('/admin-new/workflow-runs')).toBeNull();
    expect(workflowRunIdFromPathname('/admin-new/workflow-runs/%E0%A4%A')).toBeNull();
  });

  it('builds encoded canonical links', () => {
    expect(workflowRunDetailHref('run/1')).toBe('/admin-new/workflow-runs/run%2F1');
    expect(workflowRunDefinitionHref('workflow/1')).toBe('/admin-new/workflows/workflow%2F1');
    expect(workflowRunProjectHref('project/1')).toBe('/admin-new/projects/project%2F1');
    expect(workflowRunSessionHref('session/1')).toBe('/admin-new/sessions/session%2F1');
    expect(workflowRunSessionPreviewHref('session/1'))
      .toBe('/admin-new/sessions/session%2F1/preview');
  });

  it('formats structured evidence and byte counts without HTML interpretation', () => {
    expect(formatWorkflowRunJson({ value: '<script>alert(1)</script>' }))
      .toContain('<script>alert(1)</script>');
    expect(formatWorkflowRunJson(undefined)).toBe('Not provided');
    expect(formatWorkflowRunBytes(512)).toBe('512 B');
    expect(formatWorkflowRunBytes(2048)).toBe('2.0 KiB');
    expect(formatWorkflowRunBytes(2 * 1024 * 1024)).toBe('2.0 MiB');
  });
});

function workflowRun(overrides: {
  readonly state?: string;
  readonly pendingRequest?: boolean;
  readonly projectAdmission?: WorkflowRunResource['project_admission'];
  readonly admission?: WorkflowRunResource['admission'];
} = {}): WorkflowRunResource {
  return {
    id: 'run-1',
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'workflow-version-1',
    workflow_version: 'v1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    source_system: 'external_scheduler',
    source_reference: 'ticket-42',
    client_request_id: 'request-1',
    state: overrides.state ?? 'awaiting_input',
    session_id: 'session-1',
    automation_task_id: 'task-1',
    input: { approved: false },
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
    project_admission: overrides.projectAdmission ?? {
      state: 'allowed',
      reason_code: 'policy_ok',
      message: 'Workflow run allowed.',
      active_workflow_runs: 1,
      max_active_workflow_runs: 4,
      checked_at: '2026-08-07T10:00:00.000Z',
    },
    admission: overrides.admission ?? null,
    intervention: {
      pending_request: overrides.pendingRequest === false ? null : {
        request_id: 'intervention-1',
        kind: 'approval',
        prompt: 'Approve the next step',
        requested_at: '2026-08-07T10:05:00.000Z',
      },
      last_resolution: null,
    },
    runtime: {
      resume_mode: 'live_runtime',
      exact_runtime_available: true,
      hold_until: '2026-08-07T10:35:00.000Z',
      released_at: null,
      release_reason: null,
      session_state: 'ready',
    },
    labels: {},
    started_at: '2026-08-07T10:01:00.000Z',
    completed_at: null,
    events_path: '/api/v1/workflow-runs/run-1/events',
    logs_path: '/api/v1/workflow-runs/run-1/logs',
    created_at: '2026-08-07T10:00:00.000Z',
    updated_at: '2026-08-07T10:05:00.000Z',
  };
}
