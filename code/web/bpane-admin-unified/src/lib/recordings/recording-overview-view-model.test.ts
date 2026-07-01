import { describe, expect, it } from 'vitest';

import { toSessionRecordingResource } from './recording-client';
import {
  buildRecordingOverviewModel,
  recordingMatchesSearch,
  recordingOverviewRow,
} from './recording-overview-view-model';
import { sessionResource } from '$lib/test-utils/session-fixtures';

describe('recording overview view model', () => {
  it('summarizes recording metrics and row state', () => {
    const entries = [
      {
        session: sessionResource({ id: 'session-ready-123456', projectName: 'Support' }),
        recording: recording({ id: 'recording-ready-123456', sessionId: 'session-ready-123456' }),
      },
      {
        session: sessionResource({ id: 'session-active-123456', projectName: null }),
        recording: recording({
          id: 'recording-active-123456',
          sessionId: 'session-active-123456',
          state: 'recording',
          artifactAvailable: false,
          completedAt: null,
        }),
      },
      {
        session: sessionResource({ id: 'session-failed-123456', projectName: 'Support' }),
        recording: recording({
          id: 'recording-failed-123456',
          sessionId: 'session-failed-123456',
          state: 'failed',
          artifactAvailable: false,
          error: 'worker exited',
        }),
      },
    ];

    const model = buildRecordingOverviewModel(entries);

    expect(model.metrics.map((metric) => [metric.label, metric.value])).toEqual([
      ['Recordings', '3'],
      ['Downloadable', '1'],
      ['Active segments', '1'],
      ['Failed', '1'],
    ]);
    expect(model.rows[0]).toMatchObject({
      shortId: 'recording-re',
      shortSessionId: 'session-read',
      project: 'Support',
      state: 'ready',
      stateTone: 'success',
      artifactLabel: 'artifact ready',
      canDownload: true,
      downloadKind: 'recording_segment',
      downloadDescription: 'Download recording WebM',
      downloadFileName: 'bpane-recording-session-read-recording-re.webm',
    });
    expect(model.rows[1]).toMatchObject({
      project: 'Owner scoped',
      stateTone: 'warning',
      canDownload: false,
      completedAt: 'Not completed',
    });
    expect(recordingMatchesSearch(model.rows[2]!, 'worker')).toBe(true);
  });

  it('builds a row for missing ready artifacts', () => {
    const row = recordingOverviewRow({
      session: sessionResource({ id: 'session-1' }),
      recording: recording({
        id: 'recording-1',
        sessionId: 'session-1',
        artifactAvailable: false,
      }),
    });

    expect(row.stateTone).toBe('warning');
    expect(row.artifactLabel).toBe('artifact unavailable');
    expect(row.canDownload).toBe(false);
    expect(row.downloadKind).toBe('unavailable');
  });

  it('uses playback zip downloads when a session has multiple downloadable segments', () => {
    const entries = [
      {
        session: sessionResource({ id: 'session-multi-123456' }),
        recording: recording({ id: 'recording-one-123456', sessionId: 'session-multi-123456' }),
      },
      {
        session: sessionResource({ id: 'session-multi-123456' }),
        recording: recording({ id: 'recording-two-123456', sessionId: 'session-multi-123456' }),
      },
    ];

    const model = buildRecordingOverviewModel(entries);

    expect(model.rows[0]).toMatchObject({
      downloadKind: 'playback_export',
      downloadDescription: 'Download session playback ZIP',
      downloadFileName: 'bpane-recording-playback-session-mult.zip',
    });
  });
});

function recording(
  overrides: Partial<{
    readonly id: string;
    readonly sessionId: string;
    readonly state: string;
    readonly artifactAvailable: boolean;
    readonly bytes: number | null;
    readonly durationMs: number | null;
    readonly completedAt: string | null;
    readonly error: string | null;
  }> = {},
) {
  const id = overrides.id ?? 'recording-1';
  const sessionId = overrides.sessionId ?? 'session-1';
  return toSessionRecordingResource({
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
    completed_at: overrides.completedAt === undefined ? '2026-06-21T10:01:01.000Z' : overrides.completedAt,
    created_at: '2026-06-21T10:00:00.000Z',
    updated_at: '2026-06-21T10:01:01.000Z',
  });
}
