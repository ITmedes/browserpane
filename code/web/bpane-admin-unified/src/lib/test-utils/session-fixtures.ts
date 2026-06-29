import { toSessionResource, toSessionStatus } from '$lib/sessions/session-client';
import type { SessionResource, SessionStatus } from '$lib/sessions/session-types';

export function sessionResource(
  overrides: Partial<{
    readonly id: string;
    readonly state: string;
    readonly runtimeState: string;
    readonly presenceState: string;
    readonly totalClients: number;
    readonly projectName: string | null;
    readonly admissionState: string;
    readonly admissionReasonCode: string;
    readonly egressHealth: string;
    readonly queued: boolean;
    readonly stopAllowed: boolean;
  }> = {},
): SessionResource {
  return toSessionResource(sessionPayload(overrides));
}

export function sessionStatus(
  overrides: Partial<{
    readonly state: string;
    readonly runtimeState: string;
    readonly presenceState: string;
    readonly totalClients: number;
    readonly stopAllowed: boolean;
  }> = {},
): SessionStatus {
  return toSessionStatus(sessionStatusPayload(overrides));
}

export function sessionPayload(
  overrides: Partial<{
    readonly id: string;
    readonly state: string;
    readonly runtimeState: string;
    readonly presenceState: string;
    readonly totalClients: number;
    readonly projectName: string | null;
    readonly admissionState: string;
    readonly admissionReasonCode: string;
    readonly egressHealth: string;
    readonly queued: boolean;
    readonly stopAllowed: boolean;
  }> = {},
): Record<string, unknown> {
  const id = overrides.id ?? 'session-1';
  const state = overrides.state ?? 'ready';
  const totalClients = overrides.totalClients ?? 1;
  const stopAllowed = overrides.stopAllowed ?? totalClients === 0;
  const projectName = overrides.projectName === undefined ? 'Support' : overrides.projectName;
  return {
    id,
    state,
    project_id: projectName ? 'project-1' : null,
    project: projectName ? { id: 'project-1', name: projectName, state: 'active' } : null,
    admission: projectName
      ? {
          state: overrides.admissionState ?? 'allowed',
          reason_code: overrides.admissionReasonCode ?? 'policy_ok',
          message: 'Project policy allows the session.',
          checked_at: '2026-06-21T10:00:00.000Z',
        }
      : null,
    template_id: 'template-1',
    browser_context: { mode: 'reusable', context_id: 'context-1' },
    network_identity: {
      locale: 'en-US',
      languages: ['en-US'],
      timezone: 'UTC',
      user_agent: null,
      browser_identity: 'chromium',
      egress_profile_id: 'egress-1',
    },
    effective_egress: {
      profile_id: 'egress-1',
      profile_name: 'Support proxy',
      profile_state: 'ready',
      proxy_configured: true,
      proxy_auth_configured: false,
      bypass_rule_count: 1,
      custom_ca_configured: false,
      observation_mode: 'metadata_only',
      tls_interception_enabled: false,
      sensitive_log_sink_configured: false,
    },
    egress_diagnostics: {
      health: overrides.egressHealth ?? 'ready',
      proof_level: 'active_probe',
      observation_mode: 'metadata_only',
      warnings: [],
      observed_at: '2026-06-21T10:05:00.000Z',
    },
    owner_mode: 'shared',
    viewport: { width: 1280, height: 720 },
    capabilities: {
      browser_input: true,
      clipboard: true,
      audio: true,
      microphone: false,
      camera: false,
      file_transfer: true,
      resize: true,
    },
    automation_delegate: null,
    idle_timeout_sec: 300,
    labels: { suite: 'sessions' },
    integration_context: { source: 'test' },
    connect: {
      gateway_url: 'https://localhost:4433',
      transport_path: `/session/${id}`,
      auth_type: 'session_connect_ticket',
      ticket_path: `/api/v1/sessions/${id}/access-tokens`,
      compatibility_mode: 'webtransport',
    },
    runtime: {
      binding: 'docker:browser-1',
      compatibility_mode: 'docker_pool',
      cdp_endpoint: 'http://browser:9222',
    },
    status: {
      runtime_state: overrides.runtimeState ?? 'running',
      runtime_resume_mode: 'profile_restart',
      presence_state: overrides.presenceState ?? 'connected',
      connection_counts: connectionCounts(totalClients),
      stop_eligibility: stopEligibility(stopAllowed, totalClients),
    },
    queue: overrides.queued
      ? {
          queued_at: '2026-06-21T10:00:00.000Z',
          queued_for_ms: 4500,
          position: 2,
          active_sessions: 2,
          queued_sessions: 3,
          max_active_sessions: 2,
          dispatch_blocker: 'max_active_sessions',
          cancellable: true,
        }
      : null,
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
    queued_at: overrides.queued ? '2026-06-21T10:00:00.000Z' : null,
    runtime_released_at: null,
    stopped_at: null,
  };
}

export function sessionStatusPayload(
  overrides: Partial<{
    readonly state: string;
    readonly runtimeState: string;
    readonly presenceState: string;
    readonly totalClients: number;
    readonly stopAllowed: boolean;
  }> = {},
): Record<string, unknown> {
  const totalClients = overrides.totalClients ?? 1;
  const stopAllowed = overrides.stopAllowed ?? totalClients === 0;
  return {
    state: overrides.state ?? 'ready',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    admission: {
      state: 'allowed',
      reason_code: 'policy_ok',
      message: 'Project policy allows the session.',
      checked_at: '2026-06-21T10:00:00.000Z',
    },
    runtime_state: overrides.runtimeState ?? 'running',
    runtime_resume_mode: 'profile_restart',
    presence_state: overrides.presenceState ?? 'connected',
    connection_counts: connectionCounts(totalClients),
    stop_eligibility: stopEligibility(stopAllowed, totalClients),
    idle: {
      idle_timeout_sec: 300,
      idle_since: null,
      idle_deadline: null,
    },
    connections: totalClients > 0
      ? [{ connection_id: 7, role: 'browser-owner' }]
      : [],
    browser_clients: totalClients > 0 ? 1 : 0,
    viewer_clients: 0,
    recorder_clients: 0,
    max_viewers: 10,
    viewer_slots_remaining: 10,
    exclusive_browser_owner: false,
    mcp_owner: false,
    resolution: [1280, 720],
    network_identity: null,
    effective_egress: null,
    egress_diagnostics: null,
  };
}

function connectionCounts(totalClients: number): Record<string, number> {
  return {
    interactive_clients: totalClients > 0 ? 1 : 0,
    owner_clients: totalClients > 0 ? 1 : 0,
    viewer_clients: 0,
    recorder_clients: 0,
    automation_clients: 0,
    total_clients: totalClients,
  };
}

function stopEligibility(allowed: boolean, totalClients: number): Record<string, unknown> {
  return {
    allowed,
    blockers: allowed
      ? []
      : [{ kind: totalClients > 0 ? 'browser_clients' : 'runtime_busy', count: Math.max(1, totalClients) }],
  };
}
