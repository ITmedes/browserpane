import type { SessionRecordingPlaybackResource } from './recording-types';
import type { SessionConnectionCounts, SessionStopEligibility } from './control-types';

export type SessionIdleStatus = {
  readonly idle_timeout_sec: number | null;
  readonly idle_since: string | null;
  readonly idle_deadline: string | null;
};

export type SessionConnectionInfo = {
  readonly connection_id: number;
  readonly role: string;
};

export type SessionRecordingStatus = {
  readonly configured_mode: string;
  readonly format: string;
  readonly retention_sec: number | null;
  readonly state: string;
  readonly active_recording_id: string | null;
  readonly recorder_attached: boolean;
  readonly started_at: string | null;
  readonly bytes_written: number | null;
  readonly duration_ms: number | null;
};

export type SessionTelemetry = {
  readonly joins_accepted: number;
  readonly joins_rejected_viewer_cap: number;
  readonly last_join_latency_ms: number;
  readonly average_join_latency_ms: number;
  readonly max_join_latency_ms: number;
  readonly full_refresh_requests: number;
  readonly full_refresh_tiles_requested: number;
  readonly last_full_refresh_tiles: number;
  readonly max_full_refresh_tiles: number;
  readonly egress_send_stream_lock_acquires_total: number;
  readonly egress_send_stream_lock_wait_us_total: number;
  readonly egress_send_stream_lock_wait_us_average: number;
  readonly egress_send_stream_lock_wait_us_max: number;
  readonly egress_lagged_receives_total: number;
  readonly egress_lagged_frames_total: number;
};

export type SessionStatus = {
  readonly state: string;
  readonly runtime_state: string;
  readonly presence_state: string;
  readonly connection_counts: SessionConnectionCounts;
  readonly stop_eligibility: SessionStopEligibility;
  readonly idle: SessionIdleStatus;
  readonly connections: readonly SessionConnectionInfo[];
  readonly browser_clients: number;
  readonly viewer_clients: number;
  readonly recorder_clients: number;
  readonly max_viewers: number;
  readonly viewer_slots_remaining: number;
  readonly exclusive_browser_owner: boolean;
  readonly mcp_owner: boolean;
  readonly resolution: readonly [number, number];
  readonly recording: SessionRecordingStatus;
  readonly playback: SessionRecordingPlaybackResource;
  readonly telemetry: SessionTelemetry;
};
