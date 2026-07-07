import { describe, expect, it } from 'vitest';

import { buildProjectOverviewModel } from './project-overview-view-model';
import type { ProjectResource } from './project-types';

describe('buildProjectOverviewModel', () => {
  it('summarizes project states, alerts, usage, and policy for the table view', () => {
    const projects = [
      project({
        id: 'project-a',
        name: 'Support',
        alerts: 1,
        usageBudgetEnforcement: 'block_session_creation',
      }),
      project({ id: 'project-b', name: 'Archive', state: 'archived' }),
    ];

    const model = buildProjectOverviewModel(projects);

    expect(model.metrics.map((metric) => [metric.label, metric.value])).toEqual([
      ['Projects', '2'],
      ['Active', '1'],
      ['Archived', '1'],
      ['Usage alerts', '1'],
    ]);
    expect(model.rows[0]).toMatchObject({
      name: 'Support',
      stateTone: 'success',
      activeSessions: '2 / 5',
      queuedSessions: '1',
      activeWorkflowRuns: '1 / 2',
      sessionCreations: '10 / unbounded',
      runtimeUsage: '2m / unbounded',
      egressUsage: '1.0 KB / unbounded',
      retainedStorage: '2.0 KB / unbounded',
      alerts: '1 exceeded',
      alertTone: 'danger',
      policySummary: '1 restricted selectors, 2 disabled operations, blocking budgets',
      updatedAt: expect.stringContaining('2026'),
    });
    expect(model.rows[1]).toMatchObject({
      name: 'Archive',
      stateTone: 'neutral',
      alertTone: 'neutral',
      alerts: 'none',
    });
  });
});

function project(options: {
  readonly id: string;
  readonly name: string;
  readonly state?: ProjectResource['state'];
  readonly alerts?: number;
  readonly usageBudgetEnforcement?: ProjectResource['policy']['usage_budget_enforcement'];
}): ProjectResource {
  return {
    id: options.id,
    name: options.name,
    description: `${options.name} project`,
    labels: { env: 'test' },
    quotas: {
      max_active_sessions: 5,
      max_active_workflow_runs: 2,
    },
    policy: {
      allowed_session_template_ids: ['template-a'],
      allowed_egress_profile_ids: [],
      allowed_extension_ids: [],
      allowed_browser_context_ids: [],
      allowed_file_workspace_ids: [],
      allow_browser_uploads: true,
      allow_browser_downloads: false,
      allow_session_file_bindings: true,
      allow_manual_recordings: false,
      usage_budget_enforcement: options.usageBudgetEnforcement ?? 'warning_only',
    },
    state: options.state ?? 'active',
    usage: {
      project_id: options.id,
      active_sessions: 2,
      queued_sessions: 1,
      session_creations: 10,
      max_session_creations: null,
      max_active_sessions: 5,
      active_workflow_runs: 1,
      max_active_workflow_runs: 2,
      runtime_usage_ms: 120_000,
      max_runtime_usage_ms: null,
      egress_rx_bytes: 512,
      egress_tx_bytes: 512,
      egress_total_bytes: 1024,
      max_egress_total_bytes: null,
      retained_storage_bytes: 2048,
      max_retained_storage_bytes: null,
      alerts: Array.from({ length: options.alerts ?? 0 }, () => ({
        metric: 'session_creations',
        state: 'exceeded',
        current_value: 10,
        limit_value: 8,
        threshold_percent: 100,
        message: 'Session creation budget exceeded.',
      })),
      observed_at: '2026-06-11T10:00:00.000Z',
    },
    created_at: '2026-06-11T09:00:00.000Z',
    updated_at: '2026-06-11T10:00:00.000Z',
  };
}
