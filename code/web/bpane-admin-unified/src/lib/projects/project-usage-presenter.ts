import {
  formatBytes,
  formatDateTime,
  formatDuration,
  usageWithLimit,
  type ProjectTone,
} from './project-formatters';
import type {
  ProjectUsageGovernanceModel,
  ProjectUsageMetricId,
  ProjectUsagePressure,
  ProjectUsagePressureState,
} from './project-governance-types';
import type { ProjectResource } from './project-types';

type MetricInput = {
  readonly id: ProjectUsageMetricId;
  readonly label: string;
  readonly current: number;
  readonly limit: number | null;
  readonly currentLabel: string;
  readonly limitLabel: string | null;
  readonly description: (state: ProjectUsagePressureState) => string;
};

export class ProjectUsagePresenter {
  public build(project: ProjectResource): ProjectUsageGovernanceModel {
    const usage = project.usage;
    return {
      enforcement: project.policy.usage_budget_enforcement,
      enforcementLabel: project.policy.usage_budget_enforcement === 'block_session_creation'
        ? 'Block new sessions when session creation or runtime budgets are exhausted'
        : 'Warning only; usage alerts do not reject new sessions',
      metrics: [
        this.metric({
          id: 'active_sessions', label: 'Active sessions', current: usage.active_sessions,
          limit: usage.max_active_sessions ?? null, currentLabel: String(usage.active_sessions),
          limitLabel: this.numberLabel(usage.max_active_sessions),
          description: (state) => state === 'at_limit' || state === 'exceeded'
            ? 'New sessions may remain queued until project capacity becomes available.'
            : 'Concurrency admission for browser sessions in this project.',
        }),
        this.queuedSessions(usage.queued_sessions),
        this.metric({
          id: 'active_workflow_runs', label: 'Active workflow runs', current: usage.active_workflow_runs,
          limit: usage.max_active_workflow_runs ?? null, currentLabel: String(usage.active_workflow_runs),
          limitLabel: this.numberLabel(usage.max_active_workflow_runs),
          description: (state) => state === 'at_limit' || state === 'exceeded'
            ? 'New workflow runs may remain queued until project run capacity becomes available.'
            : 'Concurrency admission for workflow runs in this project.',
        }),
        this.metric({
          id: 'session_creations', label: 'Session creations', current: usage.session_creations,
          limit: usage.max_session_creations ?? null, currentLabel: String(usage.session_creations),
          limitLabel: this.numberLabel(usage.max_session_creations),
          description: () => this.budgetDescription(project, 'session creation'),
        }),
        this.metric({
          id: 'runtime_usage_ms', label: 'Runtime usage', current: usage.runtime_usage_ms,
          limit: usage.max_runtime_usage_ms ?? null,
          currentLabel: formatDuration(usage.runtime_usage_ms) ?? '0s',
          limitLabel: formatDuration(usage.max_runtime_usage_ms),
          description: () => this.budgetDescription(project, 'runtime usage'),
        }),
        this.metric({
          id: 'egress_total_bytes', label: 'Sanitized egress', current: usage.egress_total_bytes,
          limit: usage.max_egress_total_bytes ?? null,
          currentLabel: formatBytes(usage.egress_total_bytes) ?? '0 B',
          limitLabel: formatBytes(usage.max_egress_total_bytes),
          description: () => 'Advisory RX/TX byte totals only; URLs, headers, credentials, and payloads remain with the proxy.',
        }),
        this.metric({
          id: 'retained_storage_bytes', label: 'Retained storage', current: usage.retained_storage_bytes,
          limit: usage.max_retained_storage_bytes ?? null,
          currentLabel: formatBytes(usage.retained_storage_bytes) ?? '0 B',
          limitLabel: formatBytes(usage.max_retained_storage_bytes),
          description: () => 'New retained artifacts are rejected when this enforced storage quota would be exceeded.',
        }),
      ],
      alerts: usage.alerts,
      observedAt: formatDateTime(usage.observed_at),
    };
  }

  private queuedSessions(current: number): ProjectUsagePressure {
    return {
      id: 'queued_sessions',
      label: 'Queued sessions',
      current,
      limit: null,
      displayValue: String(current),
      state: 'unbounded',
      tone: current > 0 ? 'warning' : 'neutral',
      description: current > 0
        ? 'Sessions are waiting for project capacity and remain visible and cancellable.'
        : 'No project sessions are currently waiting for capacity.',
    };
  }

  private metric(input: MetricInput): ProjectUsagePressure {
    const state = this.pressureState(input.current, input.limit);
    return {
      id: input.id,
      label: input.label,
      current: input.current,
      limit: input.limit,
      displayValue: usageWithLimit(input.currentLabel, input.limitLabel),
      state,
      tone: this.pressureTone(state),
      description: input.description(state),
    };
  }

  private pressureState(current: number, limit: number | null): ProjectUsagePressureState {
    if (limit === null) return 'unbounded';
    if (current > limit) return 'exceeded';
    if (current === limit) return 'at_limit';
    if (current / limit >= 0.8) return 'approaching';
    return 'normal';
  }

  private pressureTone(state: ProjectUsagePressureState): ProjectTone {
    if (state === 'exceeded') return 'danger';
    if (state === 'at_limit' || state === 'approaching') return 'warning';
    if (state === 'normal') return 'success';
    return 'neutral';
  }

  private budgetDescription(project: ProjectResource, subject: string): string {
    return project.policy.usage_budget_enforcement === 'block_session_creation'
      ? `Exhausted ${subject} budget blocks new project sessions but does not stop existing work.`
      : `${subject[0]?.toUpperCase() ?? ''}${subject.slice(1)} alerts are advisory in warning-only mode.`;
  }

  private numberLabel(value: number | null | undefined): string | null {
    return value === null || value === undefined ? null : String(value);
  }
}
