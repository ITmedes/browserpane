import { describe, expect, it } from 'vitest';
import { IdentityAccessReviewViewModelBuilder } from './identity-access-review-view-model';
import type { IdentityAccessReviewResponse } from '../api/control-types';

const REVIEW: IdentityAccessReviewResponse = {
  principal: {
    subject: 'demo',
    issuer: 'http://localhost:8091/realms/browserpane',
    display_name: 'demo',
    client_id: 'bpane-web',
    principal_type: 'user',
  },
  generated_at: '2026-05-29T10:00:00Z',
  resource_counts: {
    projects: 1,
    service_principals: 1,
    identity_mappings: 1,
    sessions: 2,
    active_sessions: 1,
    session_templates: 1,
    browser_contexts: 1,
    egress_profiles: 1,
    credential_bindings: 0,
    file_workspaces: 1,
    workflow_definitions: 1,
    workflow_runs: 2,
    active_workflow_runs: 1,
    automation_tasks: 2,
    active_automation_tasks: 1,
    extension_definitions: 0,
    delegated_principals: 1,
  },
  projects: [
    {
      id: 'project-1',
      name: 'Support tenant',
      description: null,
      labels: { team: 'support' },
      quotas: {
        max_active_sessions: 3,
        max_active_workflow_runs: 4,
        max_retained_storage_bytes: 1048576,
      },
      state: 'active',
      usage: {
        project_id: 'project-1',
        active_sessions: 1,
        max_active_sessions: 3,
        active_workflow_runs: 2,
        max_active_workflow_runs: 4,
        retained_storage_bytes: 524288,
        max_retained_storage_bytes: 1048576,
        observed_at: '2026-05-29T10:00:00Z',
      },
      created_at: '2026-05-29T09:00:00Z',
      updated_at: '2026-05-29T10:00:00Z',
    },
  ],
  identity_mappings: [
    {
      id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f8',
      name: 'Demo project access',
      description: null,
      kind: 'user',
      issuer: 'http://localhost:8091/realms/browserpane',
      external_id: 'demo',
      claim_name: null,
      service_principal_id: null,
      project_id: 'project-1',
      labels: {},
      scopes: ['session:create'],
      state: 'active',
      last_seen_at: null,
      effective_for_principal: true,
      created_at: '2026-05-29T09:00:00Z',
      updated_at: '2026-05-29T10:00:00Z',
    },
  ],
  unmapped_principal_signals: [],
  service_principals: [
    {
      id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f7',
      name: 'BrowserPane MCP bridge',
      description: 'Bridge automation identity',
      client_id: 'bpane-mcp-bridge',
      issuer: 'http://localhost:8091/realms/browserpane',
      labels: { system: 'mcp' },
      scopes: ['session:delegate'],
      allowed_project_ids: ['project-1'],
      state: 'active',
      last_seen_at: null,
      last_delegated_at: '2026-05-29T10:00:00Z',
      delegated_session_count: 1,
      active_delegated_session_count: 1,
      delegated_session_ids: ['019df4d2-f4f7-7b00-9e0c-79683b1c82f6'],
      created_at: '2026-05-29T09:00:00Z',
      updated_at: '2026-05-29T10:00:00Z',
    },
  ],
  delegated_principals: [
    {
      client_id: 'bpane-mcp-bridge',
      issuer: 'http://localhost:8091/realms/browserpane',
      display_name: 'BrowserPane MCP bridge',
      registered: true,
      registered_service_principal_id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f7',
      state: 'active',
      session_count: 1,
      active_session_count: 1,
      session_ids: ['019df4d2-f4f7-7b00-9e0c-79683b1c82f6'],
    },
  ],
};

describe('IdentityAccessReviewViewModelBuilder', () => {
  it('formats identity review counts, projects, and delegations', () => {
    const viewModel = IdentityAccessReviewViewModelBuilder.build(REVIEW);

    expect(viewModel.principalTitle).toBe('demo');
    expect(viewModel.principalTypeLabel).toBe('User');
    expect(viewModel.metrics.find((metric) => metric.key === 'sessions')).toMatchObject({
      label: 'Sessions',
      value: '2',
      testId: 'identity-resource-sessions',
    });
    expect(viewModel.projects[0]).toMatchObject({
      name: 'Support tenant',
      activeSessions: '1/3',
      activeWorkflowRuns: '2/4',
      retainedStorage: '512 KiB / 1.0 MiB',
    });
    expect(viewModel.servicePrincipals[0]).toMatchObject({
      name: 'BrowserPane MCP bridge',
      state: 'active',
      scopes: 'session:delegate',
      delegatedSummary: '1/1 active',
      delegatedSessionIds: '019df4d2...82f6',
    });
    expect(viewModel.mappings[0]).toMatchObject({
      name: 'Demo project access',
      kind: 'User',
      externalId: 'demo',
      projectId: 'project-1',
      effective: 'effective',
      scopes: 'session:create',
    });
    expect(viewModel.delegations[0]).toMatchObject({
      clientId: 'bpane-mcp-bridge',
      displayName: 'BrowserPane MCP bridge',
      registration: 'registered 019df4d2...82f7',
      state: 'active',
      sessionSummary: '1/1 active',
      sessionIds: '019df4d2...82f6',
    });
  });
});
