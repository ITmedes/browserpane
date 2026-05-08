import { describe, expect, it, vi } from 'vitest';
import { ControlClient, type FetchLike } from './control-client';

const SESSION_ID = '019df4d2-f4f7-7b00-9e0c-79683b1c82f6';

describe('ControlClient session status routes', () => {
  it('gets the full session status snapshot', async () => {
    const fetchImpl = jsonFetch(sessionStatusPayload());
    const client = createClient(fetchImpl);

    const status = await client.getSessionStatus(SESSION_ID);

    expect(status.connections[0]).toMatchObject({ connection_id: 7, role: 'owner' });
    expect(status.recording.state).toBe('idle');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL(`http://localhost:8932/api/v1/sessions/${SESSION_ID}/status`),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('disconnects a selected live connection', async () => {
    const fetchImpl = jsonFetch(sessionStatusPayload({ connections: [] }));
    const client = createClient(fetchImpl);

    const status = await client.disconnectSessionConnection(SESSION_ID, 7);

    expect(status.connections).toEqual([]);
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL(`http://localhost:8932/api/v1/sessions/${SESSION_ID}/connections/7/disconnect`),
      expect.objectContaining({ method: 'POST' }),
    );
  });

  it('disconnects all live connections', async () => {
    const fetchImpl = jsonFetch(sessionStatusPayload({ state: 'idle', connections: [] }));
    const client = createClient(fetchImpl);

    const status = await client.disconnectAllSessionConnections(SESSION_ID);

    expect(status.state).toBe('idle');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL(`http://localhost:8932/api/v1/sessions/${SESSION_ID}/connections/disconnect-all`),
      expect.objectContaining({ method: 'POST' }),
    );
  });
});

function createClient(fetchImpl: ReturnType<typeof vi.fn<FetchLike>>): ControlClient {
  return new ControlClient({
    baseUrl: 'http://localhost:8932',
    accessTokenProvider: () => 'owner-token',
    fetchImpl,
  });
}

function jsonFetch(payload: unknown): ReturnType<typeof vi.fn<FetchLike>> {
  return vi.fn<FetchLike>(async () => {
    return new Response(JSON.stringify(payload), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  });
}

function sessionStatusPayload(overrides: {
  readonly state?: string;
  readonly connections?: readonly unknown[];
} = {}): Record<string, unknown> {
  return {
    state: overrides.state ?? 'active',
    runtime_state: 'running',
    presence_state: 'connected',
    connection_counts: counts(overrides.connections?.length ?? 1),
    stop_eligibility: { allowed: false, blockers: [{ kind: 'owner_clients', count: 1 }] },
    idle: { idle_timeout_sec: 300, idle_since: null, idle_deadline: null },
    connections: overrides.connections ?? [{ connection_id: 7, role: 'owner' }],
    browser_clients: overrides.connections?.length ?? 1,
    viewer_clients: 0,
    recorder_clients: 0,
    max_viewers: 10,
    viewer_slots_remaining: 9,
    exclusive_browser_owner: false,
    mcp_owner: false,
    resolution: [1280, 720],
    recording: recordingStatus(),
    playback: playbackStatus(),
    telemetry: telemetryStatus(),
  };
}

function counts(total: number): Record<string, number> {
  return {
    interactive_clients: total,
    owner_clients: total,
    viewer_clients: 0,
    recorder_clients: 0,
    automation_clients: 0,
    total_clients: total,
  };
}

function recordingStatus(): Record<string, unknown> {
  return {
    configured_mode: 'manual',
    format: 'webm',
    retention_sec: 86400,
    state: 'idle',
    active_recording_id: null,
    recorder_attached: false,
    started_at: null,
    bytes_written: null,
    duration_ms: null,
  };
}

function playbackStatus(): Record<string, unknown> {
  return {
    session_id: SESSION_ID,
    state: 'empty',
    segment_count: 0,
    included_segment_count: 0,
    failed_segment_count: 0,
    active_segment_count: 0,
    missing_artifact_segment_count: 0,
    included_bytes: 0,
    included_duration_ms: 0,
    manifest_path: `/api/v1/sessions/${SESSION_ID}/recording-playback/manifest`,
    export_path: `/api/v1/sessions/${SESSION_ID}/recording-playback/export`,
    generated_at: '2026-05-04T19:01:00Z',
  };
}

function telemetryStatus(): Record<string, number> {
  return {
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
  };
}
