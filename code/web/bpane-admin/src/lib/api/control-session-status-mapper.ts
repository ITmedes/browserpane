import { RecordingMapper } from './recording-mapper';
import type {
  SessionConnectionInfo,
  SessionIdleStatus,
  SessionRecordingStatus,
  SessionStatus,
  SessionTelemetry,
} from './session-status-types';
import {
  expectBoolean,
  expectNumber,
  expectRecord,
  expectString,
  optionalString,
} from './control-wire';
import type { SessionConnectionCounts, SessionStopEligibility } from './control-types';

export class ControlSessionStatusMapper {
  static toSessionStatus(payload: unknown): SessionStatus {
    const object = expectRecord(payload, 'session status');
    return {
      state: expectString(object.state, 'session status state'),
      runtime_state: expectString(object.runtime_state, 'session status runtime_state'),
      presence_state: expectString(object.presence_state, 'session status presence_state'),
      connection_counts: toConnectionCounts(object.connection_counts),
      stop_eligibility: toStopEligibility(object.stop_eligibility),
      idle: toIdleStatus(object.idle),
      connections: toConnections(object.connections),
      browser_clients: expectNumber(object.browser_clients, 'browser_clients'),
      viewer_clients: expectNumber(object.viewer_clients, 'viewer_clients'),
      recorder_clients: expectNumber(object.recorder_clients, 'recorder_clients'),
      max_viewers: expectNumber(object.max_viewers, 'max_viewers'),
      viewer_slots_remaining: expectNumber(object.viewer_slots_remaining, 'viewer_slots_remaining'),
      exclusive_browser_owner: expectBoolean(object.exclusive_browser_owner, 'exclusive_browser_owner'),
      mcp_owner: expectBoolean(object.mcp_owner, 'mcp_owner'),
      resolution: toResolution(object.resolution),
      recording: toRecordingStatus(object.recording),
      playback: RecordingMapper.toPlayback(object.playback),
      telemetry: toTelemetry(object.telemetry),
    };
  }
}

function toConnectionCounts(value: unknown): SessionConnectionCounts {
  const object = expectRecord(value, 'session status connection_counts');
  return {
    interactive_clients: expectNumber(object.interactive_clients, 'interactive_clients'),
    owner_clients: expectNumber(object.owner_clients, 'owner_clients'),
    viewer_clients: expectNumber(object.viewer_clients, 'viewer_clients'),
    recorder_clients: expectNumber(object.recorder_clients, 'recorder_clients'),
    automation_clients: expectNumber(object.automation_clients, 'automation_clients'),
    total_clients: expectNumber(object.total_clients, 'total_clients'),
  };
}

function toStopEligibility(value: unknown): SessionStopEligibility {
  const object = expectRecord(value, 'session status stop_eligibility');
  if (!Array.isArray(object.blockers)) {
    throw new Error('session stop eligibility blockers must be an array');
  }
  return {
    allowed: expectBoolean(object.allowed, 'session stop eligibility allowed'),
    blockers: object.blockers.map((blocker) => {
      const blockerObject = expectRecord(blocker, 'session stop blocker');
      return {
        kind: expectString(blockerObject.kind, 'session stop blocker kind'),
        count: expectNumber(blockerObject.count, 'session stop blocker count'),
      };
    }),
  };
}

function toIdleStatus(value: unknown): SessionIdleStatus {
  const object = expectRecord(value, 'session status idle');
  return {
    idle_timeout_sec: nullableNumber(object.idle_timeout_sec, 'idle_timeout_sec'),
    idle_since: nullableString(object.idle_since, 'idle_since'),
    idle_deadline: nullableString(object.idle_deadline, 'idle_deadline'),
  };
}

function toConnections(value: unknown): readonly SessionConnectionInfo[] {
  if (!Array.isArray(value)) {
    throw new Error('session status connections must be an array');
  }
  return value.map((connection) => {
    const object = expectRecord(connection, 'session connection');
    return {
      connection_id: expectNumber(object.connection_id, 'connection_id'),
      role: expectString(object.role, 'connection role'),
    };
  });
}

function toResolution(value: unknown): readonly [number, number] {
  if (!Array.isArray(value) || value.length !== 2) {
    throw new Error('session status resolution must contain width and height');
  }
  return [
    expectNumber(value[0], 'session status resolution width'),
    expectNumber(value[1], 'session status resolution height'),
  ];
}

function toRecordingStatus(value: unknown): SessionRecordingStatus {
  const object = expectRecord(value, 'session recording status');
  return {
    configured_mode: expectString(object.configured_mode, 'recording configured_mode'),
    format: expectString(object.format, 'recording format'),
    retention_sec: nullableNumber(object.retention_sec, 'recording retention_sec'),
    state: expectString(object.state, 'recording state'),
    active_recording_id: nullableString(object.active_recording_id, 'active_recording_id'),
    recorder_attached: expectBoolean(object.recorder_attached, 'recorder_attached'),
    started_at: nullableString(object.started_at, 'recording started_at'),
    bytes_written: nullableNumber(object.bytes_written, 'recording bytes_written'),
    duration_ms: nullableNumber(object.duration_ms, 'recording duration_ms'),
  };
}

function toTelemetry(value: unknown): SessionTelemetry {
  const object = expectRecord(value, 'session telemetry');
  return {
    joins_accepted: expectNumber(object.joins_accepted, 'joins_accepted'),
    joins_rejected_viewer_cap: expectNumber(object.joins_rejected_viewer_cap, 'joins_rejected_viewer_cap'),
    last_join_latency_ms: expectNumber(object.last_join_latency_ms, 'last_join_latency_ms'),
    average_join_latency_ms: expectNumber(object.average_join_latency_ms, 'average_join_latency_ms'),
    max_join_latency_ms: expectNumber(object.max_join_latency_ms, 'max_join_latency_ms'),
    full_refresh_requests: expectNumber(object.full_refresh_requests, 'full_refresh_requests'),
    full_refresh_tiles_requested: expectNumber(object.full_refresh_tiles_requested, 'full_refresh_tiles_requested'),
    last_full_refresh_tiles: expectNumber(object.last_full_refresh_tiles, 'last_full_refresh_tiles'),
    max_full_refresh_tiles: expectNumber(object.max_full_refresh_tiles, 'max_full_refresh_tiles'),
    egress_send_stream_lock_acquires_total: expectNumber(object.egress_send_stream_lock_acquires_total, 'egress lock acquires'),
    egress_send_stream_lock_wait_us_total: expectNumber(object.egress_send_stream_lock_wait_us_total, 'egress wait total'),
    egress_send_stream_lock_wait_us_average: expectNumber(object.egress_send_stream_lock_wait_us_average, 'egress wait average'),
    egress_send_stream_lock_wait_us_max: expectNumber(object.egress_send_stream_lock_wait_us_max, 'egress wait max'),
    egress_lagged_receives_total: expectNumber(object.egress_lagged_receives_total, 'egress lagged receives'),
    egress_lagged_frames_total: expectNumber(object.egress_lagged_frames_total, 'egress lagged frames'),
  };
}

function nullableNumber(value: unknown, label: string): number | null {
  if (value === null || value === undefined) {
    return null;
  }
  return expectNumber(value, label);
}

function nullableString(value: unknown, label: string): string | null {
  return optionalString(value, label) ?? null;
}
