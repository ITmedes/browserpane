import { describe, expect, it } from 'vitest';

import { ProjectUsagePresenter } from './project-usage-presenter';
import type { ProjectResource } from './project-types';

describe('ProjectUsagePresenter', () => {
  const presenter = new ProjectUsagePresenter();

  it('distinguishes normal, approaching, at-limit, exceeded, and unbounded usage', () => {
    const model = presenter.build(project());

    expect(metric(model, 'active_sessions')).toMatchObject({
      displayValue: '4 / 5',
      state: 'approaching',
      tone: 'warning',
    });
    expect(metric(model, 'active_workflow_runs')).toMatchObject({
      state: 'at_limit',
      tone: 'warning',
    });
    expect(metric(model, 'session_creations')).toMatchObject({
      state: 'exceeded',
      tone: 'danger',
    });
    expect(metric(model, 'runtime_usage_ms')).toMatchObject({
      state: 'normal',
      tone: 'success',
    });
    expect(metric(model, 'retained_storage_bytes')).toMatchObject({
      displayValue: '512 B / unbounded',
      state: 'unbounded',
      tone: 'neutral',
    });
  });

  it('keeps sanitized egress evidence and warning-only semantics explicit', () => {
    const model = presenter.build(project());
    const egress = metric(model, 'egress_total_bytes');

    expect(model.enforcementLabel).toContain('Warning only');
    expect(metric(model, 'session_creations').description).toContain('advisory');
    expect(egress.displayValue).toBe('3.0 KB / 4.0 KB');
    expect(egress.description).toContain('URLs, headers, credentials, and payloads');
    expect(model.alerts).toHaveLength(1);
  });

  it('explains blocking budgets without implying active work is stopped', () => {
    const model = presenter.build(project('block_session_creation'));

    expect(model.enforcementLabel).toContain('Block new sessions');
    expect(metric(model, 'runtime_usage_ms').description).toContain(
      'does not stop existing work',
    );
  });
});

function metric(
  model: ReturnType<ProjectUsagePresenter['build']>,
  id: ReturnType<ProjectUsagePresenter['build']>['metrics'][number]['id'],
) {
  const result = model.metrics.find((candidate) => candidate.id === id);
  expect(result).toBeDefined();
  return result!;
}

function project(
  enforcement: ProjectResource['policy']['usage_budget_enforcement'] = 'warning_only',
): ProjectResource {
  return {
    id: 'project-1',
    name: 'Support',
    description: 'Support browser work',
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
      usage_budget_enforcement: enforcement,
    },
    state: 'active',
    usage: {
      project_id: 'project-1',
      active_sessions: 4,
      queued_sessions: 2,
      session_creations: 11,
      max_session_creations: 10,
      max_active_sessions: 5,
      active_workflow_runs: 2,
      max_active_workflow_runs: 2,
      runtime_usage_ms: 3_600_000,
      max_runtime_usage_ms: 7_200_000,
      egress_rx_bytes: 1_024,
      egress_tx_bytes: 2_048,
      egress_total_bytes: 3_072,
      max_egress_total_bytes: 4_096,
      retained_storage_bytes: 512,
      max_retained_storage_bytes: null,
      alerts: [{
        metric: 'session_creations',
        state: 'exceeded',
        current_value: 11,
        limit_value: 10,
        threshold_percent: 100,
        message: 'Session creation budget exceeded.',
      }],
      observed_at: '2026-08-10T09:00:00.000Z',
    },
    created_at: '2026-08-10T08:00:00.000Z',
    updated_at: '2026-08-10T09:00:00.000Z',
  };
}
