import { describe, expect, it } from 'vitest';
import type { LiveBrowserSessionConnection } from '../session/browser-session-types';
import { RecordingViewModelBuilder } from './recording-view-model';

describe('RecordingViewModelBuilder', () => {
  it('enables start for connected recording-capable sessions', () => {
    const viewModel = RecordingViewModelBuilder.build({
      liveConnection: connection(),
      recording: false,
      busy: false,
      lastArtifactName: null,
      error: null,
    });

    expect(viewModel.canStart).toBe(true);
    expect(viewModel.canStop).toBe(false);
    expect(viewModel.status).toBe('idle');
  });

  it('enables stop while recording and download after an artifact exists', () => {
    const viewModel = RecordingViewModelBuilder.build({
      liveConnection: connection(),
      recording: true,
      busy: false,
      lastArtifactName: 'session.webm',
      error: null,
    });

    expect(viewModel.canStart).toBe(false);
    expect(viewModel.canStop).toBe(true);
    expect(viewModel.canDownload).toBe(true);
  });

  it('disables controls without a recording-capable handle', () => {
    const viewModel = RecordingViewModelBuilder.build({
      liveConnection: null,
      recording: false,
      busy: false,
      lastArtifactName: null,
      error: null,
    });

    expect(viewModel.canStart).toBe(false);
    expect(viewModel.note).toContain('Connect');
  });
});

function connection(): LiveBrowserSessionConnection {
  return {
    sessionId: 'session-a',
    gatewayUrl: 'https://localhost:4433/session',
    handle: {
      disconnect: () => {},
      startRecording: async () => {},
      stopRecording: async () => new Blob(),
    },
  };
}
