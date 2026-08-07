import {
  isActiveWorkflowRun,
  runNeedsAttention,
  workflowRunOverviewRow,
  type WorkflowRunOverviewRow,
} from '$lib/workflow-runs/workflow-run-overview-view-model';
import type { WorkflowRunResource } from '$lib/workflow-runs/workflow-run-types';

export type SessionAutomationMetric = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type SessionAutomationModel = {
  readonly metrics: readonly SessionAutomationMetric[];
  readonly workflowRuns: readonly WorkflowRunOverviewRow[];
};

export function buildSessionAutomationModel(
  sessionId: string,
  runs: readonly WorkflowRunResource[],
): SessionAutomationModel {
  const matchingRuns = runs
    .filter((run) => run.session_id === sessionId)
    .toSorted((left, right) => right.updated_at.localeCompare(left.updated_at));

  return {
    metrics: [
      metric('total', 'Associated runs', matchingRuns.length),
      metric('active', 'Active', matchingRuns.filter(isActiveWorkflowRun).length),
      metric('attention', 'Needs attention', matchingRuns.filter(runNeedsAttention).length),
    ],
    workflowRuns: matchingRuns.map(workflowRunOverviewRow),
  };
}

function metric(id: string, label: string, value: number): SessionAutomationMetric {
  return {
    label,
    value: String(value),
    testId: `session-automation-${id}`,
  };
}
