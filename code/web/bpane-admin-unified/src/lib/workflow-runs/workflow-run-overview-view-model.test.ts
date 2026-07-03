import { describe, expect, it } from 'vitest';

import type { WorkflowRunResource } from './workflow-run-types';
import {
  buildWorkflowRunOverviewModel,
  runNeedsAttention,
  workflowRunMatchesSearch,
  workflowRunOverviewRow,
} from './workflow-run-overview-view-model';

describe('workflow-run-overview-view-model', () => {
  it('builds metrics and row labels for active, completed, and attention states', () => {
    const active = workflowRun({ id: 'active-run', state: 'running' });
    const awaiting = workflowRun({ id: 'awaiting-run', state: 'awaiting_input', pendingInput: true });
    const failed = workflowRun({ id: 'failed-run', state: 'failed', error: 'assertion failed' });

    const model = buildWorkflowRunOverviewModel([active, awaiting, failed]);

    expect(model.metrics.map((metric) => [metric.label, metric.value])).toEqual([
      ['Workflow runs', '3'],
      ['Active', '2'],
      ['Needs attention', '2'],
      ['Completed', '1'],
    ]);
    expect(model.rows[0]).toMatchObject({
      shortId: 'active-run',
      stateTone: 'warning',
      project: 'Support',
      output: '1 produced files · 1 artifacts',
    });
    expect(model.rows[1]?.intervention).toContain('operator_input');
    expect(model.rows[2]?.stateTone).toBe('danger');
  });

  it('matches search across identifiers, project, runtime, and labels', () => {
    const row = workflowRunOverviewRow(workflowRun({
      id: 'run-abcdef1234567890',
      state: 'succeeded',
      labels: { customer: 'acme' },
    }));

    expect(workflowRunMatchesSearch(row, 'abcdef')).toBe(true);
    expect(workflowRunMatchesSearch(row, 'support')).toBe(true);
    expect(workflowRunMatchesSearch(row, 'acme')).toBe(true);
    expect(workflowRunMatchesSearch(row, 'missing')).toBe(false);
  });

  it('marks denied admissions and pending operator input as attention', () => {
    expect(runNeedsAttention(workflowRun({ state: 'succeeded' }))).toBe(false);
    expect(runNeedsAttention(workflowRun({ state: 'succeeded', pendingInput: true }))).toBe(true);
    expect(runNeedsAttention(workflowRun({ state: 'succeeded', projectAdmissionState: 'denied' }))).toBe(true);
  });
});

function workflowRun(overrides: Partial<{
  readonly id: string;
  readonly state: string;
  readonly error: string | null;
  readonly pendingInput: boolean;
  readonly labels: Readonly<Record<string, string>>;
  readonly projectAdmissionState: string;
}> = {}): WorkflowRunResource {
  const id = overrides.id ?? 'run-1';
  return {
    id,
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'workflow-version-1',
    workflow_version: 'v1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    source_system: 'manual',
    source_reference: 'admin',
    client_request_id: 'request-1',
    state: overrides.state ?? 'running',
    session_id: 'session-1',
    automation_task_id: 'task-1',
    input: {},
    output: null,
    error: overrides.error ?? null,
    artifact_refs: ['artifact-1'],
    produced_files: [{
      workspace_id: 'workspace-1',
      file_id: 'file-1',
      file_name: 'report.json',
      media_type: 'application/json',
      byte_count: 512,
      sha256_hex: 'abc123',
      content_path: '/api/v1/workflow-runs/run-1/produced-files/file-1/content',
      created_at: '2026-06-21T10:05:00.000Z',
    }],
    project_admission: {
      state: overrides.projectAdmissionState ?? 'allowed',
      reason_code: 'policy_ok',
      message: 'Workflow run allowed.',
      checked_at: '2026-06-21T10:00:00.000Z',
    },
    admission: {
      state: 'dispatched',
      reason: 'capacity_available',
      queued_at: '2026-06-21T10:00:00.000Z',
    },
    intervention: {
      pending_request: overrides.pendingInput
        ? {
            request_id: 'input-1',
            kind: 'operator_input',
            prompt: 'Approve change',
            requested_at: '2026-06-21T10:03:00.000Z',
          }
        : null,
    },
    runtime: {
      resume_mode: 'hold_exact_runtime',
      exact_runtime_available: true,
      hold_until: '2026-06-21T10:30:00.000Z',
      released_at: null,
      release_reason: null,
      session_state: 'ready',
    },
    labels: overrides.labels ?? { suite: 'workflow-runs' },
    started_at: '2026-06-21T10:01:00.000Z',
    completed_at: overrides.state === 'succeeded' || overrides.state === 'failed'
      ? '2026-06-21T10:05:00.000Z'
      : null,
    events_path: '/api/v1/workflow-runs/run-1/events',
    logs_path: '/api/v1/workflow-runs/run-1/logs',
    created_at: '2026-06-21T10:00:00.000Z',
    updated_at: '2026-06-21T10:05:00.000Z',
  };
}
