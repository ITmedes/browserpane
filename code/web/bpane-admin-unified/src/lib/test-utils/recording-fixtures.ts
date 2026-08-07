export function recordingPayload(
  overrides: Partial<{
    readonly id: string;
    readonly sessionId: string;
    readonly state: string;
    readonly artifactAvailable: boolean;
    readonly bytes: number | null;
    readonly durationMs: number | null;
    readonly error: string | null;
  }> = {},
): Record<string, unknown> {
  const id = overrides.id ?? 'recording-1';
  const sessionId = overrides.sessionId ?? 'session-1';
  return {
    id,
    session_id: sessionId,
    previous_recording_id: null,
    state: overrides.state ?? 'ready',
    format: 'webm',
    mime_type: 'video/webm',
    bytes: overrides.bytes ?? 12_345,
    duration_ms: overrides.durationMs ?? 61_000,
    error: overrides.error ?? null,
    termination_reason: 'manual_stop',
    artifact_available: overrides.artifactAvailable ?? true,
    content_path: `/api/v1/sessions/${sessionId}/recordings/${id}/content`,
    started_at: '2026-06-21T10:00:00.000Z',
    completed_at: '2026-06-21T10:01:01.000Z',
    created_at: '2026-06-21T10:00:00.000Z',
    updated_at: '2026-06-21T10:01:01.000Z',
  };
}

export function playbackPayload(
  overrides: Readonly<Record<string, unknown>> = {},
): Record<string, unknown> {
  return {
    session_id: 'session/one',
    state: 'partial',
    segment_count: 3,
    included_segment_count: 2,
    failed_segment_count: 1,
    active_segment_count: 0,
    missing_artifact_segment_count: 0,
    included_bytes: 24_690,
    included_duration_ms: 122_000,
    manifest_path: '/api/v1/sessions/session%2Fone/recording-playback/manifest',
    export_path: '/api/v1/sessions/session%2Fone/recording-playback/export',
    generated_at: '2026-08-07T10:00:00Z',
    ...overrides,
  };
}
