import type { ProjectResource } from '$lib/projects/project-types';

type ProjectFixtureOverrides = Omit<Partial<ProjectResource>, 'policy' | 'usage'> & {
  readonly policy?: Partial<ProjectResource['policy']>;
  readonly usage?: Partial<ProjectResource['usage']>;
};

export function projectResourceFixture(
  overrides: ProjectFixtureOverrides = {},
): ProjectResource {
  const id = overrides.id ?? 'project-1';
  return {
    id,
    name: 'Support',
    description: 'Support browser work',
    labels: { team: 'support' },
    quotas: { max_active_sessions: 4 },
    state: 'active',
    created_at: '2026-08-10T08:00:00.000Z',
    updated_at: '2026-08-10T09:00:00.000Z',
    ...overrides,
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
      ...overrides.policy,
    },
    usage: {
      project_id: id,
      active_sessions: 1,
      queued_sessions: 0,
      session_creations: 1,
      max_session_creations: null,
      max_active_sessions: 4,
      active_workflow_runs: 0,
      max_active_workflow_runs: null,
      runtime_usage_ms: 30_000,
      max_runtime_usage_ms: null,
      egress_rx_bytes: 0,
      egress_tx_bytes: 0,
      egress_total_bytes: 0,
      max_egress_total_bytes: null,
      retained_storage_bytes: 0,
      max_retained_storage_bytes: null,
      alerts: [],
      observed_at: '2026-08-10T09:00:00.000Z',
      ...overrides.usage,
    },
  };
}
