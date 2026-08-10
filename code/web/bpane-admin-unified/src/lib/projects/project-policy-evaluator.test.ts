import { describe, expect, it } from 'vitest';

import { ProjectPolicyEvaluator } from './project-policy-evaluator';
import type { ProjectPolicyOption, ProjectResource } from './project-types';

describe('ProjectPolicyEvaluator', () => {
  const evaluator = new ProjectPolicyEvaluator();

  it('allows owner resources when the project policy is unrestricted', () => {
    expect(evaluator.evaluateOption(project(), 'egress_profile', option())).toMatchObject({
      allowed: true,
      code: 'allowed_unrestricted',
      tone: 'neutral',
    });
  });

  it('allows owner resources and blocks project resources without a selected project', () => {
    expect(evaluator.evaluateOwnerScopeOption('egress_profile', option())).toMatchObject({
      allowed: true,
      code: 'allowed_unrestricted',
    });
    expect(evaluator.evaluateOwnerScopeOption(
      'egress_profile',
      option({ projectId: 'project-1' }),
    )).toMatchObject({ allowed: false, code: 'blocked_project_scope' });
  });

  it('distinguishes allowlisted, policy-blocked, and cross-project resources', () => {
    const restricted = project({ allowed_egress_profile_ids: ['egress-1'] });

    expect(evaluator.evaluateOption(restricted, 'egress_profile', option())).toMatchObject({
      allowed: true,
      code: 'allowed_by_policy',
    });
    expect(evaluator.evaluateOption(
      restricted,
      'egress_profile',
      option({ id: 'egress-2', name: 'Unlisted proxy' }),
    )).toMatchObject({ allowed: false, code: 'blocked_by_policy' });
    expect(evaluator.evaluateOption(
      restricted,
      'egress_profile',
      option({ id: 'egress-3', projectId: 'project-2' }),
    )).toMatchObject({ allowed: false, code: 'blocked_project_scope' });
  });

  it('rejects disabled and missing references with explicit reasons', () => {
    expect(evaluator.evaluateOption(
      project(),
      'egress_profile',
      option({ state: 'disabled' }),
    )).toMatchObject({ allowed: false, code: 'blocked_resource_state' });

    expect(evaluator.evaluateReference(
      project({ allowed_egress_profile_ids: ['missing-egress'] }),
      'egress_profile',
      'missing-egress',
      [],
    )).toMatchObject({
      allowed: false,
      code: 'missing_reference',
      tone: 'warning',
    });
  });

  it('summarizes allowlists and operation policy gates', () => {
    const governed = project({
      allowed_file_workspace_ids: ['workspace-1'],
      allow_browser_uploads: false,
      allow_manual_recordings: false,
    });

    expect(evaluator.summarizeAllowlist(governed, 'file_workspace', [])).toMatchObject({
      mode: 'restricted',
      configuredCount: 1,
      resources: [{ code: 'missing_reference' }],
    });
    expect(evaluator.operationPolicies(governed)).toEqual(expect.arrayContaining([
      expect.objectContaining({ id: 'browser_uploads', allowed: false, tone: 'warning' }),
      expect.objectContaining({ id: 'browser_downloads', allowed: true, tone: 'success' }),
      expect.objectContaining({ id: 'manual_recordings', allowed: false }),
    ]));
  });
});

function option(overrides: Partial<ProjectPolicyOption> = {}): ProjectPolicyOption {
  return {
    id: 'egress-1',
    projectId: null,
    name: 'Support proxy',
    description: null,
    state: 'ready',
    scope: 'Owner scoped',
    ...overrides,
  };
}

function project(
  policyOverrides: Partial<ProjectResource['policy']> = {},
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
      usage_budget_enforcement: 'warning_only',
      ...policyOverrides,
    },
    state: 'active',
    usage: emptyUsage(),
    created_at: '2026-08-10T08:00:00.000Z',
    updated_at: '2026-08-10T09:00:00.000Z',
  };
}

function emptyUsage(): ProjectResource['usage'] {
  return {
    project_id: 'project-1',
    active_sessions: 0,
    queued_sessions: 0,
    session_creations: 0,
    max_session_creations: null,
    max_active_sessions: null,
    active_workflow_runs: 0,
    max_active_workflow_runs: null,
    runtime_usage_ms: 0,
    max_runtime_usage_ms: null,
    egress_rx_bytes: 0,
    egress_tx_bytes: 0,
    egress_total_bytes: 0,
    max_egress_total_bytes: null,
    retained_storage_bytes: 0,
    max_retained_storage_bytes: null,
    alerts: [],
    observed_at: '2026-08-10T09:00:00.000Z',
  };
}
