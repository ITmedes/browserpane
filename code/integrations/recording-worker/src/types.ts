export type GatewaySessionState =
  | "pending"
  | "starting"
  | "ready"
  | "active"
  | "idle"
  | "stopping"
  | "stopped"
  | "failed"
  | "expired";

export type GatewayRecordingMode = "disabled" | "manual" | "always";
export type GatewayRecordingFormat = "webm";
export type GatewayRecordingState =
  | "starting"
  | "recording"
  | "finalizing"
  | "ready"
  | "failed";
export type GatewayRecordingTerminationReason =
  | "manual_stop"
  | "session_stop"
  | "idle_stop"
  | "gateway_restart"
  | "worker_exit";

export type GatewaySessionResource = {
  id: string;
  state: GatewaySessionState;
  recording: {
    mode: GatewayRecordingMode;
    format: GatewayRecordingFormat;
    retention_sec: number | null;
  };
};

export type GatewayRecordingResource = {
  id: string;
  session_id: string;
  previous_recording_id: string | null;
  state: GatewayRecordingState;
  format: GatewayRecordingFormat;
  mime_type: string | null;
  bytes: number | null;
  duration_ms: number | null;
  error: string | null;
  termination_reason: GatewayRecordingTerminationReason | null;
  artifact_available: boolean;
  content_path: string;
  started_at: string;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
};
