import { describe, expect, it } from 'vitest';

import type { BrowserContextResource } from '$lib/browser-contexts/browser-context-types';
import type { EgressProfileResource } from '$lib/egress-profiles/egress-profile-types';
import type { FileWorkspaceResource } from '$lib/file-workspaces/file-workspace-types';
import type { RecordingCatalogEntry, SessionRecordingResource } from '$lib/recordings/recording-types';
import { sessionResource } from '$lib/test-utils/session-fixtures';
import type { WorkflowRunResource } from '$lib/workflow-runs/workflow-run-types';
import type { WorkflowDefinitionResource } from '$lib/workflows/workflow-types';
import {
  buildDashboardOverviewModel,
  type DashboardSnapshot,
} from './dashboard-overview-view-model';

describe('dashboard-overview-view-model', () => {
  it('builds metrics, quick links, attention items, and recent activity', () => {
    const session = sessionResource({
      id: 'session-1234567890',
      totalClients: 2,
      stopAllowed: true,
    });
    const snapshot = dashboardSnapshot({
      sessions: [session],
      projects: [projectWithAlert()],
      browserContexts: [browserContext({ storageExceeded: true })],
      egressProfiles: [egressProfile({ health: 'blocked' })],
      fileWorkspaces: [fileWorkspace()],
      workflows: [workflowDefinition()],
      workflowRuns: [workflowRun({ state: 'awaiting_input', pendingInput: true })],
      recordings: [recordingEntry(session, recording({ state: 'failed', artifact_available: false, error: 'recorder stopped' }))],
    });

    const model = buildDashboardOverviewModel(snapshot, [
      { resource: 'File workspaces', message: 'HTTP 503', href: '/admin-new/files/workspaces' },
    ]);

    expect(model.metrics.map((metric) => [metric.label, metric.value])).toEqual([
      ['Active sessions', '1'],
      ['Workflow runs', '1'],
      ['Project alerts', '1'],
      ['Resource catalog', '4'],
    ]);
    expect(model.quickLinks.map((link) => [link.label, link.value])).toContainEqual(['Recordings', '1']);
    expect(model.attentionItems.map((item) => item.title)).toEqual([
      'File workspaces unavailable',
      'Support runtime usage ms',
      'Workflow run run-1',
      'Recording recording-1',
      'Reusable Support',
      'TLS Observer',
    ]);
    expect(model.recentActivity[0]?.title).toBe('Recording recording-1');
  });

  it('keeps empty catalogs calm and navigable', () => {
    const model = buildDashboardOverviewModel(dashboardSnapshot());

    expect(model.attentionItems).toHaveLength(0);
    expect(model.recentActivity).toHaveLength(0);
    expect(model.quickLinks).toHaveLength(8);
    expect(model.metrics.find((metric) => metric.testId === 'dashboard-metric-sessions')).toMatchObject({
      value: '0',
      tone: 'neutral',
    });
  });
});

function dashboardSnapshot(overrides: Partial<DashboardSnapshot> = {}): DashboardSnapshot {
  return {
    sessions: [],
    projects: [],
    browserContexts: [],
    egressProfiles: [],
    fileWorkspaces: [],
    workflows: [],
    workflowRuns: [],
    recordings: [],
    ...overrides,
  };
}

function projectWithAlert(): DashboardSnapshot['projects'][number] {
  return {
    id: 'project-1',
    name: 'Support',
    state: 'active',
    usage: {
      active_sessions: 1,
      queued_sessions: 0,
      active_workflow_runs: 1,
      alerts: [{
        metric: 'runtime_usage_ms',
        state: 'approaching_limit',
        message: 'Runtime usage is approaching the configured limit.',
      }],
    },
  };
}

function browserContext(overrides: Partial<{ readonly storageExceeded: boolean }> = {}): BrowserContextResource {
  return {
    id: 'context-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    name: 'Reusable Support',
    description: null,
    labels: {},
    persistence_mode: 'reusable',
    retention_sec: null,
    retention_expires_at: null,
    max_profile_storage_bytes: 1024,
    state: 'ready',
    usage: {
      visible_session_count: 1,
      active_runtime_session_count: 0,
      active_runtime_session_id: null,
      profile_storage_bytes: 2048,
      profile_storage_limit_exceeded: overrides.storageExceeded ?? false,
    },
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
    last_used_at: null,
    deleted_at: null,
  };
}

function egressProfile(
  overrides: Partial<{ readonly health: EgressProfileResource['diagnostics']['health'] }> = {},
): EgressProfileResource {
  const health = overrides.health ?? 'ready';
  return {
    id: 'egress-1',
    project_id: null,
    project: null,
    name: 'TLS Observer',
    description: null,
    labels: {},
    proxy: null,
    bypass_rules: [],
    custom_ca: null,
    traffic_observation: { mode: 'tls_intercept', sensitive_log_sink_ref: null, sensitive_log_sink_display_name: null },
    state: 'ready',
    effective: {
      proxy_configured: false,
      proxy_auth_configured: false,
      bypass_rule_count: 0,
      custom_ca_configured: false,
      observation_mode: 'tls_intercept',
      tls_interception_enabled: false,
      sensitive_log_sink_configured: false,
    },
    diagnostics: {
      profile_id: 'egress-1',
      profile_name: 'TLS Observer',
      profile_state: 'ready',
      health,
      observation_mode: 'tls_intercept',
      proof_level: 'configuration',
      runtime_binding: null,
      runtime_assignment: null,
      proxy_configured: false,
      proxy_auth_configured: false,
      bypass_rule_count: 0,
      custom_ca_configured: false,
      tls_interception_enabled: false,
      sensitive_log_sink_configured: false,
      proof: {
        profile_resolved: true,
        profile_ready: true,
        profile_reachability_collected: false,
        profile_reachability_healthy: false,
        profile_reachability_observed_at: null,
        profile_reachability_failure: null,
        proxy_launch_config_expected: false,
        bypass_rules_expected: 0,
        custom_ca_launch_config_expected: false,
        tls_interception_expected: false,
        sensitive_log_sink_declared: false,
        runtime_launch_observed: false,
        active_probe_collected: false,
        observed_public_ip: null,
        observed_tls_issuer: null,
        last_failure_reason: null,
      },
      warnings: [],
      observed_at: '2026-06-21T10:00:00.000Z',
    },
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}

function fileWorkspace(): FileWorkspaceResource {
  return {
    id: 'workspace-1',
    project_id: null,
    project: null,
    name: 'Downloads',
    description: null,
    labels: {},
    files_path: '/api/v1/file-workspaces/workspace-1/files',
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}

function workflowDefinition(): WorkflowDefinitionResource {
  return {
    id: 'workflow-1',
    name: 'BrowserPane Tour',
    description: null,
    labels: {},
    latest_version: 'v1',
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}

function workflowRun(
  overrides: Partial<{ readonly state: string; readonly pendingInput: boolean }> = {},
): WorkflowRunResource {
  return {
    id: 'run-1',
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'workflow-version-1',
    workflow_version: 'v1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    source_system: 'manual',
    source_reference: 'admin',
    client_request_id: 'request-1',
    state: overrides.state ?? 'running',
    session_id: 'session-1',
    automation_task_id: 'task-1',
    input: {},
    output: null,
    error: null,
    artifact_refs: [],
    source_snapshot: null,
    extensions: [],
    credential_bindings: [],
    workspace_inputs: [],
    produced_files: [],
    recordings: [],
    retention: { logs_expire_at: null, output_expire_at: null },
    project_admission: null,
    admission: null,
    intervention: {
      pending_request: overrides.pendingInput
        ? {
            request_id: 'request-1',
            kind: 'operator_input',
            prompt: 'Approve next workflow step',
            requested_at: '2026-06-21T10:10:00.000Z',
          }
        : null,
    },
    runtime: null,
    labels: {},
    started_at: '2026-06-21T10:01:00.000Z',
    completed_at: null,
    events_path: '/api/v1/workflow-runs/run-1/events',
    logs_path: '/api/v1/workflow-runs/run-1/logs',
    created_at: '2026-06-21T10:00:00.000Z',
    updated_at: '2026-06-21T10:10:00.000Z',
  };
}

function recording(overrides: Partial<SessionRecordingResource> = {}): SessionRecordingResource {
  return {
    id: 'recording-1',
    session_id: 'session-1',
    previous_recording_id: null,
    state: 'ready',
    format: 'webm',
    mime_type: 'video/webm',
    bytes: 512,
    duration_ms: 3000,
    error: null,
    termination_reason: null,
    artifact_available: true,
    content_path: '/api/v1/sessions/session-1/recordings/recording-1/content',
    started_at: '2026-06-21T10:02:00.000Z',
    completed_at: '2026-06-21T10:05:00.000Z',
    created_at: '2026-06-21T10:02:00.000Z',
    updated_at: '2026-06-21T10:12:00.000Z',
    ...overrides,
  };
}

function recordingEntry(
  session: RecordingCatalogEntry['session'],
  resource: SessionRecordingResource,
): RecordingCatalogEntry {
  return { session, recording: resource };
}
