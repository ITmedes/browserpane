import { describe, expect, it } from 'vitest';

import { buildProjectInspectorModel } from './project-inspector-view-model';
import type { ProjectResource } from './project-types';

describe('buildProjectInspectorModel', () => {
  it('formats header, identity, usage, and generated alerts', () => {
    const model = buildProjectInspectorModel(project());

    expect(model).toMatchObject({
      id: '11111111-1111-4111-8111-111111111111',
      name: 'Support',
      state: 'active',
      stateTone: 'success',
    });
    expect(model.identityRows).toContainEqual({
      label: 'Project ID',
      value: '11111111-1111-4111-8111-111111111111',
    });
    expect(model.usageRows).toContainEqual({
      label: 'Active sessions',
      value: '1 / 4',
    });
    expect(model.alerts[0]).toMatchObject({
      label: 'session_creations exceeded',
      value: 'Session creation budget exceeded.',
    });
  });
});

function project(): ProjectResource {
  return {
    id: '11111111-1111-4111-8111-111111111111',
    name: 'Support',
    description: 'Support browser work',
    labels: { team: 'support' },
    quotas: { max_active_sessions: 4 },
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
    },
    state: 'active',
    usage: {
      project_id: '11111111-1111-4111-8111-111111111111',
      active_sessions: 1,
      queued_sessions: 0,
      session_creations: 10,
      max_session_creations: 8,
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
      alerts: [{
        metric: 'session_creations',
        state: 'exceeded',
        current_value: 10,
        limit_value: 8,
        threshold_percent: 100,
        message: 'Session creation budget exceeded.',
      }],
      observed_at: '2026-06-11T10:00:00.000Z',
    },
    created_at: '2026-06-11T09:00:00.000Z',
    updated_at: '2026-06-11T10:00:00.000Z',
  };
}
