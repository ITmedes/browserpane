import {
  formatDateTime,
  type ProjectTone,
} from '$lib/projects/project-formatters';
import type { WorkflowRunResource } from './workflow-run-types';

export type WorkflowRunOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly runs: readonly WorkflowRunResource[] };

export type WorkflowRunOverviewMetric = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type WorkflowRunOverviewRow = {
  readonly id: string;
  readonly shortId: string;
  readonly workflowId: string;
  readonly workflowVersion: string;
  readonly state: string;
  readonly stateTone: ProjectTone;
  readonly sessionId: string;
  readonly shortSessionId: string;
  readonly project: string;
  readonly admission: string;
  readonly runtime: string;
  readonly intervention: string;
  readonly output: string;
  readonly source: string;
  readonly error: string;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly terminalAt: string;
  readonly badges: readonly string[];
};

export type WorkflowRunOverviewModel = {
  readonly metrics: readonly WorkflowRunOverviewMetric[];
  readonly rows: readonly WorkflowRunOverviewRow[];
};

const ACTIVE_STATES = new Set(['pending', 'queued', 'starting', 'running', 'awaiting_input', 'cancelling']);
const TERMINAL_STATES = new Set(['succeeded', 'failed', 'cancelled', 'timed_out']);

export function buildWorkflowRunOverviewModel(
  runs: readonly WorkflowRunResource[],
): WorkflowRunOverviewModel {
  return {
    metrics: [
      metric('total', 'Workflow runs', runs.length),
      metric('active', 'Active', runs.filter((run) => isActiveWorkflowRun(run)).length),
      metric('attention', 'Needs attention', runs.filter(runNeedsAttention).length),
      metric('completed', 'Completed', runs.filter((run) => TERMINAL_STATES.has(run.state)).length),
    ],
    rows: runs.map(workflowRunOverviewRow),
  };
}

export function workflowRunOverviewRow(run: WorkflowRunResource): WorkflowRunOverviewRow {
  const project = run.project?.name ?? run.project_id ?? 'Owner scoped';
  const admission = admissionLabel(run);
  const runtime = runtimeLabel(run);
  const intervention = interventionLabel(run);
  const output = outputLabel(run);
  const source = sourceLabel(run);
  const error = run.error?.trim() || 'No error';
  return {
    id: run.id,
    shortId: shortIdentifier(run.id),
    workflowId: run.workflow_definition_id,
    workflowVersion: run.workflow_version,
    state: run.state,
    stateTone: workflowRunStateTone(run),
    sessionId: run.session_id,
    shortSessionId: shortIdentifier(run.session_id),
    project,
    admission,
    runtime,
    intervention,
    output,
    source,
    error,
    createdAt: formatDateTime(run.created_at),
    updatedAt: formatDateTime(run.updated_at),
    terminalAt: run.completed_at
      ? formatDateTime(run.completed_at)
      : run.started_at
        ? `Started ${formatDateTime(run.started_at)}`
        : `Created ${formatDateTime(run.created_at)}`,
    badges: [
      run.state,
      run.workflow_definition_id,
      run.workflow_version,
      run.session_id,
      project,
      admission,
      runtime,
      intervention,
      output,
      source,
      error,
      ...Object.entries(run.labels).map(([key, value]) => `${key}=${value}`),
    ],
  };
}

export function workflowRunMatchesSearch(
  row: WorkflowRunOverviewRow,
  normalizedQuery: string,
): boolean {
  if (!normalizedQuery) {
    return true;
  }
  return [
    row.id,
    row.shortId,
    row.workflowId,
    row.workflowVersion,
    row.state,
    row.sessionId,
    row.shortSessionId,
    row.project,
    row.admission,
    row.runtime,
    row.intervention,
    row.output,
    row.source,
    row.error,
    row.createdAt,
    row.updatedAt,
    row.terminalAt,
    ...row.badges,
  ].some((value) => value.toLowerCase().includes(normalizedQuery));
}

export function isActiveWorkflowRun(run: WorkflowRunResource): boolean {
  return ACTIVE_STATES.has(run.state);
}

export function runNeedsAttention(run: WorkflowRunResource): boolean {
  return run.state === 'failed'
    || run.state === 'timed_out'
    || run.state === 'awaiting_input'
    || Boolean(run.error)
    || Boolean(run.intervention.pending_request)
    || run.admission?.state === 'queued'
    || run.project_admission?.state === 'denied';
}

export function workflowRunStateTone(run: WorkflowRunResource): ProjectTone {
  if (run.state === 'succeeded') {
    return 'success';
  }
  if (['failed', 'timed_out', 'cancelled'].includes(run.state)) {
    return 'danger';
  }
  if (runNeedsAttention(run) || isActiveWorkflowRun(run)) {
    return 'warning';
  }
  return 'neutral';
}

function admissionLabel(run: WorkflowRunResource): string {
  if (run.admission) {
    return `${run.admission.state}: ${run.admission.reason}`;
  }
  if (run.project_admission) {
    return `${run.project_admission.state}: ${run.project_admission.reason_code}`;
  }
  return 'Admission not reported';
}

function runtimeLabel(run: WorkflowRunResource): string {
  if (!run.runtime) {
    return 'Runtime not reported';
  }
  const exact = run.runtime.exact_runtime_available ? 'exact runtime' : 'profile resume';
  const session = run.runtime.session_state ? ` · ${run.runtime.session_state}` : '';
  return `${run.runtime.resume_mode} · ${exact}${session}`;
}

function interventionLabel(run: WorkflowRunResource): string {
  const request = run.intervention.pending_request;
  if (!request) {
    return 'No operator input pending';
  }
  return `${request.kind}: ${request.prompt ?? request.request_id}`;
}

function outputLabel(run: WorkflowRunResource): string {
  const produced = run.produced_files.length;
  const artifactRefs = run.artifact_refs.length;
  if (produced > 0 && artifactRefs > 0) {
    return `${produced} produced files · ${artifactRefs} artifacts`;
  }
  if (produced > 0) {
    return `${produced} produced files`;
  }
  if (artifactRefs > 0) {
    return `${artifactRefs} artifacts`;
  }
  return 'No produced files';
}

function sourceLabel(run: WorkflowRunResource): string {
  const system = run.source_system ?? 'manual';
  const reference = run.source_reference ?? run.client_request_id ?? 'no source reference';
  return `${system}: ${reference}`;
}

function shortIdentifier(value: string): string {
  return value.length <= 12 ? value : value.slice(0, 12);
}

function metric(key: string, label: string, value: number): WorkflowRunOverviewMetric {
  return {
    label,
    value: String(value),
    testId: `workflow-runs-metric-${key}`,
  };
}
