import { describe, expect, it } from 'vitest';
import type { EgressDiagnosticsResource, SessionResource } from '../api/control-types';
import type { SessionStatus } from '../api/session-status-types';
import { SessionViewModelBuilder } from './session-view-model';

const EGRESS_DIAGNOSTICS: EgressDiagnosticsResource = {
  profile_id: '019df7be-6222-7b00-8c86-9e1f3f8d4a73',
  profile_name: 'EU support egress',
  profile_state: 'ready',
  health: 'ready',
  observation_mode: 'tls_intercept',
  proof_level: 'runtime_launch_metadata',
  runtime_binding: 'docker_runtime_pool',
  runtime_assignment: 'ready',
  proxy_configured: true,
  proxy_auth_configured: false,
  bypass_rule_count: 2,
  custom_ca_configured: true,
  tls_interception_enabled: true,
  sensitive_log_sink_configured: true,
  proof: {
    profile_resolved: true,
    profile_ready: true,
    profile_reachability_collected: false,
    profile_reachability_healthy: false,
    profile_reachability_observed_at: null,
    profile_reachability_failure: null,
    proxy_launch_config_expected: true,
    bypass_rules_expected: 2,
    custom_ca_launch_config_expected: true,
    tls_interception_expected: true,
    sensitive_log_sink_declared: true,
    runtime_launch_observed: true,
    active_probe_collected: false,
    observed_public_ip: null,
    observed_tls_issuer: null,
    last_failure_reason: null,
  },
  warnings: [],
  observed_at: '2026-05-04T19:01:00Z',
};

const SESSION: SessionResource = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  state: 'active',
  project_id: '019df811-91a5-7b00-9fe5-93403ea57f19',
  project: {
    id: '019df811-91a5-7b00-9fe5-93403ea57f19',
    name: 'Support tenant',
    state: 'active',
  },
  admission: {
    state: 'allowed',
    reason_code: 'project_quota_available',
    message: 'Project admission allowed.',
    project_id: '019df811-91a5-7b00-9fe5-93403ea57f19',
    active_sessions: 1,
    max_active_sessions: 2,
    checked_at: '2026-05-04T19:00:00Z',
  },
  template_id: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
  browser_context: {
    mode: 'reusable',
    context_id: '019df7be-6222-7b00-8c86-9e1f3f8d4a72',
  },
  network_identity: {
    locale: 'de-DE',
    languages: ['de-DE', 'en-US'],
    timezone: 'Europe/Berlin',
    geolocation: { latitude: 52.52, longitude: 13.405, accuracy_meters: 100 },
    user_agent: null,
    browser_identity: 'desktop-chromium-stable',
    egress_profile_id: '019df7be-6222-7b00-8c86-9e1f3f8d4a73',
  },
  effective_egress: {
    profile_id: '019df7be-6222-7b00-8c86-9e1f3f8d4a73',
    profile_name: 'EU support egress',
    profile_state: 'ready',
    proxy_configured: true,
    proxy_auth_configured: false,
    bypass_rule_count: 2,
    custom_ca_configured: true,
    observation_mode: 'tls_intercept',
    tls_interception_enabled: true,
    sensitive_log_sink_configured: true,
  },
  egress_diagnostics: EGRESS_DIAGNOSTICS,
  owner_mode: 'shared',
  idle_timeout_sec: 1800,
  labels: { case: '1234', purpose: 'import-repro' },
  integration_context: { ticket: 'INC-1234' },
  connect: {
    gateway_url: 'https://localhost:4433',
    transport_path: '/session',
    auth_type: 'session_connect_ticket',
    compatibility_mode: 'session_runtime_pool',
  },
  runtime: {
    binding: 'docker_runtime_pool',
    compatibility_mode: 'session_runtime_pool',
  },
  status: {
    runtime_state: 'running',
    runtime_resume_mode: 'exact_live',
    presence_state: 'connected',
    connection_counts: {
      interactive_clients: 1,
      owner_clients: 1,
      viewer_clients: 0,
      recorder_clients: 0,
      automation_clients: 0,
      total_clients: 1,
    },
    stop_eligibility: {
      allowed: false,
      blockers: [{ kind: 'owner_clients', count: 1 }],
    },
  },
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
};

const BROWSER_CONTEXT = {
  id: '019df7be-6222-7b00-8c86-9e1f3f8d4a72',
  name: 'Support profile',
  description: null,
  labels: { team: 'support' },
  persistence_mode: 'reusable',
  state: 'ready',
  created_at: '2026-05-04T18:30:00Z',
  updated_at: '2026-05-04T18:30:00Z',
  last_used_at: null,
  deleted_at: null,
} as const;

const TEMPLATE = {
  id: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
  name: 'Support triage',
  description: null,
  labels: { team: 'support' },
  defaults: {
    owner_mode: 'collaborative',
    idle_timeout_sec: 1800,
    labels: { team: 'support' },
    network_identity: null,
  },
  version: 1,
  created_at: '2026-05-04T18:00:00Z',
  updated_at: '2026-05-04T18:00:00Z',
};

const STATUS: SessionStatus = {
  state: 'active',
  project_id: '019df811-91a5-7b00-9fe5-93403ea57f19',
  project: {
    id: '019df811-91a5-7b00-9fe5-93403ea57f19',
    name: 'Support tenant',
    state: 'active',
  },
  admission: {
    state: 'allowed',
    reason_code: 'project_quota_available',
    message: 'Project admission allowed.',
    project_id: '019df811-91a5-7b00-9fe5-93403ea57f19',
    active_sessions: 1,
    max_active_sessions: 2,
    checked_at: '2026-05-04T19:00:00Z',
  },
  runtime_state: 'running',
  runtime_resume_mode: 'exact_live',
  presence_state: 'connected',
  connection_counts: SESSION.status.connection_counts,
  stop_eligibility: SESSION.status.stop_eligibility,
  idle: { idle_timeout_sec: 300, idle_since: null, idle_deadline: null },
  connections: [{ connection_id: 7, role: 'owner' }],
  browser_clients: 1,
  viewer_clients: 0,
  recorder_clients: 0,
  max_viewers: 10,
  viewer_slots_remaining: 9,
  exclusive_browser_owner: false,
  mcp_owner: true,
  resolution: [1280, 720],
  network_identity: SESSION.network_identity!,
  effective_egress: SESSION.effective_egress!,
  egress_diagnostics: EGRESS_DIAGNOSTICS,
  recording: {
    configured_mode: 'manual',
    format: 'webm',
    retention_sec: 86400,
    state: 'idle',
    active_recording_id: null,
    recorder_attached: false,
    started_at: null,
    bytes_written: null,
    duration_ms: null,
  },
  playback: {
    session_id: SESSION.id,
    state: 'empty',
    segment_count: 0,
    included_segment_count: 0,
    failed_segment_count: 0,
    active_segment_count: 0,
    missing_artifact_segment_count: 0,
    included_bytes: 0,
    included_duration_ms: 0,
    manifest_path: `/api/v1/sessions/${SESSION.id}/recording-playback/manifest`,
    export_path: `/api/v1/sessions/${SESSION.id}/recording-playback/export`,
    generated_at: '2026-05-04T19:01:00Z',
  },
  telemetry: {
    joins_accepted: 1,
    joins_rejected_viewer_cap: 0,
    last_join_latency_ms: 4,
    average_join_latency_ms: 4,
    max_join_latency_ms: 4,
    full_refresh_requests: 0,
    full_refresh_tiles_requested: 0,
    last_full_refresh_tiles: 0,
    max_full_refresh_tiles: 0,
    egress_send_stream_lock_acquires_total: 0,
    egress_send_stream_lock_wait_us_total: 0,
    egress_send_stream_lock_wait_us_average: 0,
    egress_send_stream_lock_wait_us_max: 0,
    egress_lagged_receives_total: 0,
    egress_lagged_frames_total: 0,
  },
};

describe('SessionViewModelBuilder', () => {
  it('maps sessions to compact list rows', () => {
    const viewModel = SessionViewModelBuilder.list({
      sessions: [SESSION],
      sessionTemplates: [TEMPLATE],
      browserContexts: [BROWSER_CONTEXT],
      selectedSessionId: SESSION.id,
      authenticated: true,
      loading: false,
      error: null,
    });

    expect(viewModel.sessions[0]).toMatchObject({
      id: SESSION.id,
      shortId: '019df4d2...82f6',
      lifecycle: 'active',
      runtime: 'running',
      presence: 'connected',
      clients: 1,
      project: 'Support tenant (019df811...7f19)',
      projectId: SESSION.project_id,
      admission: 'allowed | project_quota_available 1/2',
      template: 'Support triage (019df5c8...21c0)',
      templateId: TEMPLATE.id,
      browserContext: 'Support profile (019df7be...4a72)',
      browserContextId: BROWSER_CONTEXT.id,
      networkIdentity: 'de-DE | Europe/Berlin | de-DE/en-US | geo 52.52,13.405 | desktop-chromium-stable',
      egress: 'EU support egress | ready | proxy | TLS inspect | log sink | custom CA | 2 bypass',
      mcpDelegation: 'MCP not delegated',
      labels: 'case=1234, purpose=import-repro',
    });
    expect(viewModel.selectedSession).toMatchObject({
      id: SESSION.id,
      ownerMode: 'shared',
      runtimeBinding: 'docker_runtime_pool',
      canJoin: true,
    });
  });

  it('disables destructive lifecycle actions while connected', () => {
    const viewModel = SessionViewModelBuilder.detail({
      session: SESSION,
      browserContexts: [BROWSER_CONTEXT],
      connected: true,
      loading: false,
      error: null,
    });

    expect(viewModel.canStop).toBe(false);
    expect(viewModel.canKill).toBe(false);
    expect(viewModel.hint).toContain('Disconnect');
  });

  it('adds full status facts and connection controls when status is loaded', () => {
    const viewModel = SessionViewModelBuilder.detail({
      session: SESSION,
      sessionTemplates: [TEMPLATE],
      browserContexts: [BROWSER_CONTEXT],
      status: STATUS,
      connected: false,
      loading: false,
      error: null,
    });

    expect(viewModel.facts).toContainEqual({
      label: 'mcp owner',
      value: 'yes',
      testId: 'session-mcp-owner',
    });
    expect(viewModel.facts).toContainEqual({
      label: 'project',
      value: 'Support tenant (019df811...7f19)',
      testId: 'session-project',
    });
    expect(viewModel.facts).toContainEqual({
      label: 'admission',
      value: 'allowed | project_quota_available 1/2',
      testId: 'session-admission',
    });
    expect(viewModel.facts).toContainEqual({
      label: 'template',
      value: 'Support triage (019df5c8...21c0)',
      testId: 'session-template',
    });
    expect(viewModel.facts).toContainEqual({
      label: 'browser context',
      value: 'Support profile (019df7be...4a72)',
      testId: 'session-browser-context',
    });
    expect(viewModel.facts).toContainEqual({
      label: 'labels',
      value: 'case=1234, purpose=import-repro',
      testId: 'session-labels',
    });
    expect(viewModel.facts).toContainEqual({
      label: 'integration',
      value: 'ticket=INC-1234',
      testId: 'session-integration-context',
    });
    expect(viewModel.facts).toContainEqual({
      label: 'network identity',
      value: 'de-DE | Europe/Berlin | de-DE/en-US | geo 52.52,13.405 | desktop-chromium-stable',
      testId: 'session-network-identity',
    });
    expect(viewModel.facts).toContainEqual({
      label: 'egress',
      value: 'EU support egress | ready | proxy | TLS inspect | log sink | custom CA | 2 bypass',
      testId: 'session-effective-egress',
    });
    expect(viewModel.connections).toEqual([{
      id: 7,
      label: '#7',
      role: 'owner',
      canDisconnect: true,
    }]);
    expect(viewModel.canStop).toBe(false);
    expect(viewModel.canKill).toBe(false);
    expect(viewModel.canRelease).toBe(false);
    expect(viewModel.canDisconnectAll).toBe(true);
  });

  it('disables lifecycle actions when remote status reports live clients', () => {
    const viewModel = SessionViewModelBuilder.detail({
      session: {
        ...SESSION,
        status: {
          ...SESSION.status,
          stop_eligibility: { allowed: true, blockers: [] },
        },
      },
      status: {
        ...STATUS,
        stop_eligibility: { allowed: true, blockers: [] },
      },
      connected: false,
      loading: false,
      error: null,
    });

    expect(viewModel.canStop).toBe(false);
    expect(viewModel.canKill).toBe(false);
    expect(viewModel.canRelease).toBe(false);
    expect(viewModel.hint).toContain('Disconnect');
  });

  it('surfaces queue details and the queued-session cancel action', () => {
    const queuedAt = '2026-05-04T19:02:00Z';
    const queuedSession: SessionResource = {
      ...SESSION,
      state: 'queued',
      queued_at: queuedAt,
      queue: {
        queued_at: queuedAt,
        queued_for_ms: 125000,
        position: 2,
        active_sessions: 1,
        queued_sessions: 3,
        max_active_sessions: 1,
        dispatch_blocker: 'earlier_queued_session',
        cancellable: true,
      },
      status: {
        ...SESSION.status,
        runtime_state: 'queued',
        presence_state: 'disconnected',
        connection_counts: {
          interactive_clients: 0,
          owner_clients: 0,
          viewer_clients: 0,
          recorder_clients: 0,
          automation_clients: 0,
          total_clients: 0,
        },
        stop_eligibility: { allowed: true, blockers: [] },
      },
    };

    const viewModel = SessionViewModelBuilder.detail({
      session: queuedSession,
      connected: false,
      loading: false,
      error: null,
    });

    expect(viewModel.facts).toContainEqual({
      label: 'queue age',
      value: '2m 5s',
      testId: 'session-queue-age',
    });
    expect(viewModel.facts).toContainEqual({
      label: 'queue position',
      value: '2/3',
      testId: 'session-queue-position',
    });
    expect(viewModel.facts).toContainEqual({
      label: 'queue blocker',
      value: 'earlier_queued_session',
      testId: 'session-queue-blocker',
    });
    expect(viewModel.canCancelQueue).toBe(true);
    expect(viewModel.canStop).toBe(false);
    expect(viewModel.canKill).toBe(false);
    expect(viewModel.canRelease).toBe(false);
  });

  it('exposes runtime release only for disconnected runtime candidates', () => {
    const releasableSession: SessionResource = {
      ...SESSION,
      state: 'idle',
      status: {
        ...SESSION.status,
        presence_state: 'idle',
        connection_counts: {
          interactive_clients: 0,
          owner_clients: 0,
          viewer_clients: 0,
          recorder_clients: 0,
          automation_clients: 0,
          total_clients: 0,
        },
        stop_eligibility: { allowed: true, blockers: [] },
      },
    };

    const viewModel = SessionViewModelBuilder.detail({
      session: releasableSession,
      connected: false,
      loading: false,
      error: null,
    });

    expect(viewModel.canRelease).toBe(true);
    expect(viewModel.facts).toContainEqual({
      label: 'resume',
      value: 'exact_live',
      testId: 'session-runtime-resume-mode',
    });
  });

  it('allows start actions for stopped sessions', () => {
    const stoppedSession: SessionResource = {
      ...SESSION,
      state: 'stopped',
      stopped_at: '2026-05-04T19:05:00Z',
    };

    const viewModel = SessionViewModelBuilder.list({
      sessions: [stoppedSession],
      browserContexts: [BROWSER_CONTEXT],
      selectedSessionId: stoppedSession.id,
      authenticated: true,
      loading: false,
      error: null,
    });

    expect(viewModel.selectedSession?.canJoin).toBe(true);
  });
});
