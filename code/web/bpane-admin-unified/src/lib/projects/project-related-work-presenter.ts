import type { SessionResource } from '$lib/sessions/session-types';
import type { WorkflowRunResource } from '$lib/workflow-runs/workflow-run-types';

import type { ProjectTone } from './project-formatters';
import type {
  ProjectRelatedWorkItem,
  ProjectRelatedWorkModel,
} from './project-governance-types';

export class ProjectRelatedWorkPresenter {
  public build(
    projectId: string,
    sessions: readonly SessionResource[],
    workflowRuns: readonly WorkflowRunResource[],
  ): ProjectRelatedWorkModel {
    const projectSessions = sessions
      .filter((session) => session.project_id === projectId)
      .map((session) => this.sessionItem(session))
      .sort(compareWorkItems);
    const projectWorkflowRuns = workflowRuns
      .filter((run) => run.project_id === projectId)
      .map((run) => this.workflowRunItem(run))
      .sort(compareWorkItems);

    return {
      sessions: projectSessions,
      workflowRuns: projectWorkflowRuns,
      queuedSessions: projectSessions.filter(isQueued).length,
      queuedWorkflowRuns: projectWorkflowRuns.filter(isQueued).length,
    };
  }

  private sessionItem(session: SessionResource): ProjectRelatedWorkItem {
    const admission = session.admission ?? null;
    return {
      kind: 'session',
      id: session.id,
      state: session.state,
      href: `/admin-new/sessions/${encodeURIComponent(session.id)}`,
      admissionState: admission?.state ?? null,
      reasonCode: admission?.reason_code ?? session.queue?.dispatch_blocker ?? null,
      message: admission?.message ?? null,
      queuedAt: session.queue?.queued_at ?? session.queued_at ?? null,
      queuePosition: session.queue?.position ?? null,
      updatedAt: session.updated_at,
      tone: session.queue || session.queued_at
        ? 'warning'
        : workTone(session.state, admission?.state ?? null),
    };
  }

  private workflowRunItem(run: WorkflowRunResource): ProjectRelatedWorkItem {
    const projectAdmission = run.project_admission ?? null;
    const runtimeAdmission = run.admission ?? null;
    return {
      kind: 'workflow_run',
      id: run.id,
      state: run.state,
      href: `/admin-new/workflow-runs/${encodeURIComponent(run.id)}`,
      admissionState: projectAdmission?.state ?? runtimeAdmission?.state ?? null,
      reasonCode: projectAdmission?.reason_code ?? runtimeAdmission?.reason ?? null,
      message: projectAdmission?.message ?? null,
      queuedAt: runtimeAdmission?.queued_at ?? null,
      queuePosition: null,
      updatedAt: run.updated_at,
      tone: workTone(
        run.state,
        projectAdmission?.state ?? runtimeAdmission?.state ?? null,
      ),
    };
  }
}

function compareWorkItems(left: ProjectRelatedWorkItem, right: ProjectRelatedWorkItem): number {
  if (isQueued(left) !== isQueued(right)) {
    return isQueued(left) ? -1 : 1;
  }
  return Date.parse(right.updatedAt) - Date.parse(left.updatedAt);
}

function isQueued(item: ProjectRelatedWorkItem): boolean {
  return item.queuedAt !== null || item.state === 'queued' || item.admissionState === 'queued';
}

function workTone(state: string, admissionState: string | null): ProjectTone {
  if (admissionState === 'denied' || ['failed', 'error'].includes(state)) {
    return 'danger';
  }
  if (admissionState === 'queued' || state === 'queued') {
    return 'warning';
  }
  if (['ready', 'running', 'active', 'awaiting_input'].includes(state)) {
    return 'success';
  }
  return 'neutral';
}
