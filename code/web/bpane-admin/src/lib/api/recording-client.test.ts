import { describe, expect, it, vi } from 'vitest';
import { ControlClient, type FetchLike } from './control-client';

const SESSION_ID = '019df4d2-f4f7-7b00-9e0c-79683b1c82f6';
const RECORDING_ID = '019df4d2-f4f7-7b00-9e0c-79683b1c82f7';

const RECORDING = {
  id: RECORDING_ID,
  session_id: SESSION_ID,
  previous_recording_id: null,
  state: 'ready',
  format: 'webm',
  mime_type: 'video/webm',
  bytes: 4096,
  duration_ms: 2500,
  error: null,
  termination_reason: 'manual_stop',
  artifact_available: true,
  content_path: `/api/v1/sessions/${SESSION_ID}/recordings/${RECORDING_ID}/content`,
  started_at: '2026-05-04T19:00:00Z',
  completed_at: '2026-05-04T19:01:00Z',
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
};

const PLAYBACK = {
  session_id: SESSION_ID,
  state: 'ready',
  segment_count: 1,
  included_segment_count: 1,
  failed_segment_count: 0,
  active_segment_count: 0,
  missing_artifact_segment_count: 0,
  included_bytes: 4096,
  included_duration_ms: 2500,
  manifest_path: `/api/v1/sessions/${SESSION_ID}/recording-playback/manifest`,
  export_path: `/api/v1/sessions/${SESSION_ID}/recording-playback/export`,
  generated_at: '2026-05-04T19:02:00Z',
};

describe('ControlClient recording APIs', () => {
  it('lists retained session recordings with owner bearer auth', async () => {
    const fetchImpl = jsonFetch({ recordings: [RECORDING] });
    const client = newClient(fetchImpl);

    const response = await client.listSessionRecordings(SESSION_ID);

    expect(response.recordings[0]?.content_path).toBe(RECORDING.content_path);
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL(`http://localhost:8932/api/v1/sessions/${SESSION_ID}/recordings`),
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({ authorization: 'Bearer owner-token' }),
      }),
    );
  });

  it('loads playback summary and downloads retained artifacts', async () => {
    const fetchImpl = vi.fn<FetchLike>(async (input) => {
      const path = new URL(input.toString()).pathname;
      if (path.endsWith('/recording-playback')) {
        return jsonResponse(PLAYBACK);
      }
      return new Response('artifact-bytes', { status: 200 });
    });
    const client = newClient(fetchImpl);

    const playback = await client.getSessionRecordingPlayback(SESSION_ID);
    const blob = await client.downloadSessionRecordingContent(RECORDING);

    expect(playback.export_path).toBe(PLAYBACK.export_path);
    expect(blob.size).toBe('artifact-bytes'.length);
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      new URL(`http://localhost:8932${RECORDING.content_path}`),
      expect.objectContaining({ method: 'GET' }),
    );
  });
});

function newClient(fetchImpl: ReturnType<typeof vi.fn<FetchLike>>): ControlClient {
  return new ControlClient({
    baseUrl: 'http://localhost:8932',
    accessTokenProvider: () => 'owner-token',
    fetchImpl,
  });
}

function jsonFetch(payload: unknown): ReturnType<typeof vi.fn<FetchLike>> {
  return vi.fn<FetchLike>(async () => jsonResponse(payload));
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
