import { describe, expect, it } from 'vitest';

import { sessionResource } from '$lib/test-utils/session-fixtures';
import { playbackPayload, recordingPayload } from '$lib/test-utils/recording-fixtures';
import { toSessionRecordingPlaybackResource, toSessionRecordingResource } from './recording-client';
import { buildSessionRecordingDetailModel } from './session-recording-detail-view-model';

describe('buildSessionRecordingDetailModel', () => {
  it('selects the retained WebM for one downloadable segment', () => {
    const recording = toSessionRecordingResource(recordingPayload());
    const playback = toSessionRecordingPlaybackResource(playbackPayload({
      session_id: 'session-1',
      state: 'ready',
      segment_count: 1,
      included_segment_count: 1,
      failed_segment_count: 0,
      included_bytes: 12_345,
      included_duration_ms: 61_000,
    }));

    const model = buildSessionRecordingDetailModel(
      sessionResource({ id: 'session-1', recordingMode: 'always' }),
      [recording],
      playback,
    );

    expect(model).toMatchObject({
      policyEnabled: true,
      canDisablePolicy: true,
      playbackState: 'ready',
      playbackTone: 'success',
      downloadKind: 'segment',
      downloadLabel: 'Download WebM',
      downloadableRecording: recording,
    });
    expect(model.downloadFileName).toBe('bpane-recording-session-1.webm');
  });

  it('selects a playback ZIP when multiple segments are included', () => {
    const recordings = [
      toSessionRecordingResource(recordingPayload({ id: 'recording-1' })),
      toSessionRecordingResource(recordingPayload({ id: 'recording-2' })),
      toSessionRecordingResource(recordingPayload({ id: 'recording-3', state: 'failed', artifactAvailable: false })),
    ];
    const playback = toSessionRecordingPlaybackResource(playbackPayload({ session_id: 'session-1' }));

    const model = buildSessionRecordingDetailModel(sessionResource({ id: 'session-1' }), recordings, playback);

    expect(model).toMatchObject({
      playbackState: 'partial',
      playbackTone: 'warning',
      playbackSummary: '2/3 segments included',
      downloadKind: 'export',
      downloadLabel: 'Download playback ZIP',
      downloadFileName: 'bpane-recording-playback-session-1.zip',
    });
    expect(model.rows[2]).toMatchObject({
      state: 'failed',
      stateTone: 'danger',
      artifact: 'unavailable',
    });
  });

  it('keeps evidence visible when playback is unavailable and gates terminal policy changes', () => {
    const recording = toSessionRecordingResource(recordingPayload({ state: 'recording', artifactAvailable: false }));

    const model = buildSessionRecordingDetailModel(
      sessionResource({ id: 'session-1', state: 'killed', recordingMode: 'always' }),
      [recording],
      null,
    );

    expect(model).toMatchObject({
      canEnablePolicy: false,
      canDisablePolicy: false,
      playbackState: 'unavailable',
      canDownload: false,
      downloadKind: 'none',
    });
    expect(model.rows[0]?.stateTone).toBe('warning');
  });
});
