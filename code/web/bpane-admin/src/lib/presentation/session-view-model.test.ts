import { describe, expect, it } from 'vitest';
import type { SessionResource } from '../api/control-types';
import type { SessionStatus } from '../api/session-status-types';
import { SessionViewModelBuilder } from './session-view-model';

const SESSION: SessionResource = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  state: 'active',
  owner_mode: 'shared',
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

const STATUS: SessionStatus = {
  state: 'active',
  runtime_state: 'running',
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
      selectedSessionId: SESSION.id,
      authenticated: true,
      loading: false,
      error: null,
    });

    expect(viewModel.sessions[0]).toMatchObject({
      id: SESSION.id,
      lifecycle: 'active',
      runtime: 'running',
      presence: 'connected',
      clients: 1,
    });
  });

  it('disables destructive lifecycle actions while connected', () => {
    const viewModel = SessionViewModelBuilder.detail({
      session: SESSION,
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
    expect(viewModel.connections).toEqual([{
      id: 7,
      label: '#7',
      role: 'owner',
      canDisconnect: true,
    }]);
    expect(viewModel.canDisconnectAll).toBe(true);
  });
});
