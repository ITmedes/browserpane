import {
  formatDateTime,
  formatDuration,
  type ProjectTone,
} from '$lib/projects/project-formatters';
import type {
  WorkflowRunEventResource,
  WorkflowRunLogResource,
  WorkflowRunProducedFileResource,
  WorkflowRunResource,
} from './workflow-run-types';

export type WorkflowRunDetailLoadState =
  | { readonly status: 'loading'; readonly runId: string }
  | { readonly status: 'error'; readonly runId: string; readonly message: string }
  | { readonly status: 'ready'; readonly run: WorkflowRunResource };

export type WorkflowRunEvidenceState<T> =
  | { readonly status: 'idle' | 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly items: readonly T[] };

export type WorkflowRunDetailEvidenceState = {
  readonly events: WorkflowRunEvidenceState<WorkflowRunEventResource>;
  readonly logs: WorkflowRunEvidenceState<WorkflowRunLogResource>;
  readonly producedFiles: WorkflowRunEvidenceState<WorkflowRunProducedFileResource>;
};

export type WorkflowRunControlAvailability = {
  readonly canCancel: boolean;
  readonly canResolveIntervention: boolean;
  readonly cancelReason: string;
  readonly interventionReason: string;
};

export type WorkflowRunDetailFact = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type WorkflowRunDetailModel = {
  readonly stateTone: ProjectTone;
  readonly projectLabel: string;
  readonly projectHref: string | null;
  readonly sourceLabel: string;
  readonly runtimeLabel: string;
  readonly admissionLabel: string;
  readonly admissionEvidence: WorkflowRunAdmissionEvidence;
  readonly facts: readonly WorkflowRunDetailFact[];
  readonly controls: WorkflowRunControlAvailability;
};

export type WorkflowRunAdmissionEvidence = {
  readonly state: string | null;
  readonly tone: ProjectTone;
  readonly reasonCode: string | null;
  readonly message: string | null;
  readonly checkedAt: string | null;
  readonly facts: readonly WorkflowRunDetailFact[];
  readonly queueReason: string | null;
  readonly queuedAt: string | null;
};

const CANCELLABLE_STATES = new Set(['pending', 'queued', 'starting', 'running', 'awaiting_input']);
const TERMINAL_STATES = new Set(['succeeded', 'failed', 'cancelled', 'timed_out']);

export function buildWorkflowRunDetailModel(run: WorkflowRunResource): WorkflowRunDetailModel {
  const projectId = run.project?.id ?? run.project_id ?? run.project_admission?.project_id ?? null;
  const projectLabel = run.project?.name ?? projectId ?? 'Owner scoped';
  const sourceLabel = [
    run.source_system ?? 'manual',
    run.source_reference ?? run.client_request_id ?? 'No source reference',
  ].join(' / ');
  const runtimeLabel = run.runtime
    ? `${run.runtime.resume_mode} / ${run.runtime.exact_runtime_available ? 'exact runtime' : 'profile restart'}`
    : 'No retained runtime';
  const admissionLabel = run.project_admission
    ? `${run.project_admission.state}: ${run.project_admission.reason_code}`
    : run.admission
      ? `${run.admission.state}: ${run.admission.reason}`
      : 'No admission decision';
  return {
    stateTone: workflowRunDetailTone(run.state),
    projectLabel,
    projectHref: projectId ? workflowRunProjectHref(projectId) : null,
    sourceLabel,
    runtimeLabel,
    admissionLabel,
    admissionEvidence: workflowRunAdmissionEvidence(run),
    controls: workflowRunControlAvailability(run),
    facts: [
      fact('State', run.state, 'workflow-run-detail-state'),
      fact('Workflow version', run.workflow_version, 'workflow-run-detail-version'),
      fact('Project', projectLabel, 'workflow-run-detail-project'),
      fact('Runtime', runtimeLabel, 'workflow-run-detail-runtime'),
      fact('Created', formatDateTime(run.created_at), 'workflow-run-detail-created-at'),
      fact('Updated', formatDateTime(run.updated_at), 'workflow-run-detail-updated-at'),
    ],
  };
}

export function workflowRunAdmissionEvidence(
  run: WorkflowRunResource,
): WorkflowRunAdmissionEvidence {
  const admission = run.project_admission;
  const facts: WorkflowRunDetailFact[] = [];
  if (admission) {
    addAdmissionFact(facts, 'Active workflow runs', admission.active_workflow_runs,
      admission.max_active_workflow_runs, 'workflow-run-admission-active-workflows');
    addAdmissionFact(facts, 'Active sessions', admission.active_sessions,
      admission.max_active_sessions, 'workflow-run-admission-active-sessions');
    addAdmissionFact(facts, 'Session creations', admission.session_creations,
      admission.max_session_creations, 'workflow-run-admission-session-creations');
    addAdmissionFact(facts, 'Creation window', admission.session_creations_in_window,
      admission.max_session_creations_per_window, 'workflow-run-admission-creation-window',
      admission.session_creation_window_sec ? ` / ${admission.session_creation_window_sec}s` : '');
    if ((admission.runtime_usage_ms !== null && admission.runtime_usage_ms !== undefined)
      || (admission.max_runtime_usage_ms !== null && admission.max_runtime_usage_ms !== undefined)) {
      facts.push(fact(
        'Runtime usage',
        `${formatDuration(admission.runtime_usage_ms) ?? 'Unknown'} / ${formatDuration(admission.max_runtime_usage_ms) ?? 'Unbounded'}`,
        'workflow-run-admission-runtime-usage',
      ));
    }
  }
  return {
    state: admission?.state ?? null,
    tone: admissionStateTone(admission?.state ?? null),
    reasonCode: admission?.reason_code ?? null,
    message: admission?.message ?? null,
    checkedAt: admission ? formatDateTime(admission.checked_at) : null,
    facts,
    queueReason: run.admission?.reason ?? null,
    queuedAt: run.admission ? formatDateTime(run.admission.queued_at) : null,
  };
}

export function workflowRunControlAvailability(
  run: WorkflowRunResource,
): WorkflowRunControlAvailability {
  const canCancel = CANCELLABLE_STATES.has(run.state);
  const hasPendingIntervention = run.state === 'awaiting_input'
    && Boolean(run.intervention.pending_request);
  return {
    canCancel,
    canResolveIntervention: hasPendingIntervention,
    cancelReason: canCancel
      ? 'Cancellation is available for this active run.'
      : TERMINAL_STATES.has(run.state)
        ? 'Terminal workflow runs cannot be cancelled.'
        : `Cancellation is unavailable while the run is ${run.state}.`,
    interventionReason: hasPendingIntervention
      ? 'The workflow is waiting for operator input.'
      : run.state === 'awaiting_input'
        ? 'The run does not expose a pending intervention request.'
        : `Intervention controls require awaiting_input state; this run is ${run.state}.`,
  };
}

export function workflowRunDetailTone(state: string): ProjectTone {
  if (state === 'succeeded') {
    return 'success';
  }
  if (state === 'failed' || state === 'timed_out') {
    return 'danger';
  }
  if (state === 'awaiting_input' || state === 'queued' || state === 'cancelling') {
    return 'warning';
  }
  if (state === 'running' || state === 'starting' || state === 'pending') {
    return 'neutral';
  }
  return 'neutral';
}

export function workflowRunIdFromPathname(pathname: string): string | null {
  const match = pathname.match(/\/(?:workflow-runs|runs)\/([^/]+)\/?$/);
  if (!match?.[1]) {
    return null;
  }
  try {
    return decodeURIComponent(match[1]);
  } catch {
    return null;
  }
}

export function workflowRunDetailHref(runId: string): string {
  return `/admin-new/workflow-runs/${encodeURIComponent(runId)}`;
}

export function workflowRunSessionHref(sessionId: string): string {
  return `/admin-new/sessions/${encodeURIComponent(sessionId)}`;
}

export function workflowRunSessionPreviewHref(sessionId: string): string {
  return `/admin-new/sessions/${encodeURIComponent(sessionId)}/preview`;
}

export function workflowRunDefinitionHref(workflowId: string): string {
  return `/admin-new/workflows/${encodeURIComponent(workflowId)}`;
}

export function workflowRunProjectHref(projectId: string): string {
  return `/admin-new/projects/${encodeURIComponent(projectId)}`;
}

export function formatWorkflowRunJson(value: unknown): string {
  if (value === undefined) {
    return 'Not provided';
  }
  return JSON.stringify(value, null, 2) ?? 'Not provided';
}

export function formatWorkflowRunBytes(value: number): string {
  if (value < 1024) {
    return `${value} B`;
  }
  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KiB`;
  }
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
}

function fact(label: string, value: string, testId: string): WorkflowRunDetailFact {
  return { label, value, testId };
}

function admissionStateTone(state: string | null): ProjectTone {
  if (state === 'allowed') return 'success';
  if (state === 'queued') return 'warning';
  if (state === 'rejected') return 'danger';
  return 'neutral';
}

function addAdmissionFact(
  facts: WorkflowRunDetailFact[],
  label: string,
  current: number | null | undefined,
  limit: number | null | undefined,
  testId: string,
  suffix = '',
): void {
  if (current === null || current === undefined) {
    if (limit === null || limit === undefined) return;
    facts.push(fact(label, `Unknown / ${limit}${suffix}`, testId));
    return;
  }
  facts.push(fact(label, `${current} / ${limit ?? 'Unbounded'}${suffix}`, testId));
}
