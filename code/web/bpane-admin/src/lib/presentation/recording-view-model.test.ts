import { describe, expect, it } from 'vitest';
import type { SessionRecordingPlaybackResource, SessionRecordingResource } from '../api/recording-types';
import type { LiveBrowserSessionConnection } from '../session/browser-session-types';
import { RecordingViewModelBuilder, type RecordingViewModelInput } from './recording-view-model';

describe('RecordingViewModelBuilder', () => {
  it('enables start for connected recording-capable sessions', () => {
    const viewModel = RecordingViewModelBuilder.build(input({
      liveConnection: connection(),
      selectedSessionId: 'session-a',
      recording: false,
    }));

    expect(viewModel.canStart).toBe(true);
    expect(viewModel.canStop).toBe(false);
    expect(viewModel.status).toBe('idle');
  });

  it('enables stop while recording and download after an artifact exists', () => {
    const viewModel = RecordingViewModelBuilder.build(input({
      liveConnection: connection(),
      selectedSessionId: 'session-a',
      recording: true,
      lastArtifactName: 'session.webm',
    }));

    expect(viewModel.canStart).toBe(false);
    expect(viewModel.canStop).toBe(true);
    expect(viewModel.canDownload).toBe(true);
  });

  it('disables controls without a recording-capable handle', () => {
    const viewModel = RecordingViewModelBuilder.build(input());

    expect(viewModel.canStart).toBe(false);
    expect(viewModel.note).toContain('Connect');
  });

  it('disables local recording when the live browser belongs to another session', () => {
    const viewModel = RecordingViewModelBuilder.build(input({
      liveConnection: connection('session-a'),
      selectedSessionId: 'session-b',
    }));

    expect(viewModel.canStart).toBe(false);
    expect(viewModel.sessionLabel).toBe('session-b');
  });

  it('summarizes retained recordings and playback export state', () => {
    const viewModel = RecordingViewModelBuilder.build(input({
      selectedSessionId: 'session-a',
      libraryLoaded: true,
      recordings: [recording()],
      playback: playback(),
    }));

    expect(viewModel.libraryStatus).toBe('Library: 1 segment');
    expect(viewModel.playbackStatus).toContain('ready');
    expect(viewModel.canDownloadPlaybackExport).toBe(true);
    expect(viewModel.segments[0]?.metadata).toContain('Size: 4.0 KB');
  });

  it('blocks retained artifact downloads while another recording artifact is downloading', () => {
    const viewModel = RecordingViewModelBuilder.build(input({
      selectedSessionId: 'session-a',
      libraryLoaded: true,
      recordings: [recording({ id: 'recording-a' }), recording({ id: 'recording-b' })],
      playback: playback({ included_segment_count: 2, segment_count: 2 }),
      downloadingRecordingId: 'recording-a',
    }));

    expect(viewModel.segments[0]?.downloadLabel).toBe('Downloading...');
    expect(viewModel.segments[1]?.canDownload).toBe(false);
    expect(viewModel.canDownloadPlaybackExport).toBe(false);
  });
});

function connection(sessionId = 'session-a'): LiveBrowserSessionConnection {
  return {
    sessionId,
    gatewayUrl: 'https://localhost:4433/session',
    handle: {
      disconnect: () => {},
      startRecording: async () => {},
      stopRecording: async () => new Blob(),
    },
  };
}

function input(overrides: Partial<RecordingViewModelInput> = {}): RecordingViewModelInput {
  return {
    liveConnection: null,
    selectedSessionId: null,
    recording: false,
    busy: false,
    lastArtifactName: null,
    error: null,
    recordings: [],
    playback: null,
    libraryLoading: false,
    libraryLoaded: false,
    libraryError: null,
    downloadingRecordingId: null,
    downloadingPlayback: false,
    ...overrides,
  };
}

function recording(overrides: Partial<SessionRecordingResource> = {}): SessionRecordingResource {
  return {
    id: 'recording-a',
    session_id: 'session-a',
    previous_recording_id: null,
    state: 'ready',
    format: 'webm',
    mime_type: 'video/webm',
    bytes: 4096,
    duration_ms: 2500,
    error: null,
    termination_reason: 'manual_stop',
    artifact_available: true,
    content_path: '/api/v1/sessions/session-a/recordings/recording-a/content',
    started_at: '2026-05-04T19:00:00Z',
    completed_at: '2026-05-04T19:01:00Z',
    created_at: '2026-05-04T19:00:00Z',
    updated_at: '2026-05-04T19:01:00Z',
    ...overrides,
  };
}

function playback(
  overrides: Partial<SessionRecordingPlaybackResource> = {},
): SessionRecordingPlaybackResource {
  return {
    session_id: 'session-a',
    state: 'ready',
    segment_count: 1,
    included_segment_count: 1,
    failed_segment_count: 0,
    active_segment_count: 0,
    missing_artifact_segment_count: 0,
    included_bytes: 4096,
    included_duration_ms: 2500,
    manifest_path: '/api/v1/sessions/session-a/recording-playback/manifest',
    export_path: '/api/v1/sessions/session-a/recording-playback/export',
    generated_at: '2026-05-04T19:02:00Z',
    ...overrides,
  };
}
