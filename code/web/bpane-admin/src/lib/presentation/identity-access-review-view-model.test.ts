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
        max_session_creations: 8,
        max_session_creations_per_window: 2,
        session_creation_window_sec: 3600,
      },
      policy: {
        allowed_session_template_ids: ['template-1'],
        allowed_egress_profile_ids: [],
        allowed_extension_ids: ['extension-1'],
        allowed_browser_context_ids: ['context-1'],
        usage_budget_enforcement: 'block_session_creation',
      },
      state: 'active',
      usage: {
        project_id: 'project-1',
        active_sessions: 1,
        queued_sessions: 2,
        session_creations: 9,
        max_session_creations: 8,
        max_active_sessions: 3,
        active_workflow_runs: 2,
        max_active_workflow_runs: 4,
        runtime_usage_ms: 5400000,
        egress_rx_bytes: 1048576,
        egress_tx_bytes: 1048576,
        egress_total_bytes: 2097152,
        retained_storage_bytes: 524288,
        max_retained_storage_bytes: 1048576,
        alerts: [
          {
            metric: 'session_creations',
            state: 'exceeded',
            current_value: 9,
            limit_value: 8,
            threshold_percent: 100,
            message: 'Project session creation count exceeded the configured soft budget.',
          },
        ],
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
      queuedSessions: '2',
      sessionCreations: '9/8, 2 / 1h',
      activeWorkflowRuns: '2/4',
      runtimeUsage: '90m',
      egressUsage: '2.0 MiB',
      retainedStorage: '512 KiB / 1.0 MiB',
      alerts: '1 exceeded',
      policy: '1 templates, 1 extensions, 1 contexts, budget blocks session creation',
    });
    expect(viewModel.servicePrincipals[0]).toMatchObject({
      name: 'BrowserPane MCP bridge',
      state: 'active',
      scopes: 'session:delegate',
      projects: 'Support tenant (project-1)',
      delegatedSummary: '1/1 active',
      delegatedSessionIds: '019df4d2...82f6',
    });
    expect(viewModel.mappings[0]).toMatchObject({
      name: 'Demo project access',
      kind: 'User',
      externalId: 'demo',
      projectId: 'Support tenant (project-1)',
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

  it('formats safe group and claim mapping review state', () => {
    const review: IdentityAccessReviewResponse = {
      ...REVIEW,
      resource_counts: {
        ...REVIEW.resource_counts,
        identity_mappings: 2,
      },
      identity_mappings: [
        {
          ...REVIEW.identity_mappings[0]!,
          id: '019df4d2-f4f7-7b00-9e0c-79683b1c82a1',
          name: 'Acme support group',
          kind: 'group',
          external_id: 'customer-acme-support',
          scopes: ['session:create', 'session:delegate'],
          effective_for_principal: true,
        },
        {
          ...REVIEW.identity_mappings[0]!,
          id: '019df4d2-f4f7-7b00-9e0c-79683b1c82a2',
          name: 'Acme tenant claim',
          kind: 'claim',
          claim_name: 'tenant',
          external_id: 'acme',
          state: 'disabled',
          scopes: [],
          effective_for_principal: false,
        },
      ],
      unmapped_principal_signals: [
        {
          kind: 'group',
          issuer: REVIEW.principal.issuer,
          external_id: 'customer-beta-support',
          claim_name: 'groups',
          display_name: null,
          reason: 'group_without_project_mapping',
        },
        {
          kind: 'claim',
          issuer: REVIEW.principal.issuer,
          external_id: 'beta',
          claim_name: 'tenant',
          display_name: null,
          reason: 'safe_claim_without_project_mapping',
        },
      ],
    };

    const viewModel = IdentityAccessReviewViewModelBuilder.build(review);

    expect(viewModel.mappings).toMatchObject([
      {
        name: 'Acme support group',
        kind: 'Group',
        externalId: 'customer-acme-support',
        projectId: 'Support tenant (project-1)',
        state: 'active',
        effective: 'effective',
        scopes: 'session:create, session:delegate',
      },
      {
        name: 'Acme tenant claim',
        kind: 'Claim',
        externalId: 'tenant=acme',
        projectId: 'Support tenant (project-1)',
        state: 'disabled',
        effective: 'not effective',
        scopes: 'no scopes',
      },
    ]);
    expect(viewModel.unmappedSignals).toMatchObject([
      {
        key: `group:${REVIEW.principal.issuer}:groups:customer-beta-support`,
        kind: 'Group',
        externalId: 'groups=customer-beta-support',
        reason: 'Safe group claim has no active project mapping',
      },
      {
        key: `claim:${REVIEW.principal.issuer}:tenant:beta`,
        kind: 'Claim',
        externalId: 'tenant=beta',
        reason: 'Safe claim value has no active project mapping',
      },
    ]);
  });
});
