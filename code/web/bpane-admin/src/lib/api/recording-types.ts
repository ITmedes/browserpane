export type SessionRecordingResource = {
  readonly id: string;
  readonly session_id: string;
  readonly previous_recording_id: string | null;
  readonly state: string;
  readonly format: string;
  readonly mime_type: string | null;
  readonly bytes: number | null;
  readonly duration_ms: number | null;
  readonly error: string | null;
  readonly termination_reason: string | null;
  readonly artifact_available: boolean;
  readonly content_path: string;
  readonly started_at: string;
  readonly completed_at: string | null;
  readonly created_at: string;
  readonly updated_at: string;
};

export type SessionRecordingListResponse = {
  readonly recordings: readonly SessionRecordingResource[];
};

export type SessionRecordingPlaybackResource = {
  readonly session_id: string;
  readonly state: string;
  readonly segment_count: number;
  readonly included_segment_count: number;
  readonly failed_segment_count: number;
  readonly active_segment_count: number;
  readonly missing_artifact_segment_count: number;
  readonly included_bytes: number;
  readonly included_duration_ms: number;
  readonly manifest_path: string;
  readonly export_path: string;
  readonly generated_at: string;
};
