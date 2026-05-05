import type { LiveBrowserSessionConnection } from '../session/browser-session-types';

export type RecordingViewModel = {
  readonly status: string;
  readonly sessionLabel: string;
  readonly artifactLabel: string;
  readonly note: string;
  readonly canStart: boolean;
  readonly canStop: boolean;
  readonly canDownload: boolean;
  readonly busy: boolean;
  readonly error: string | null;
};

export type RecordingViewModelInput = {
  readonly liveConnection: LiveBrowserSessionConnection | null;
  readonly recording: boolean;
  readonly busy: boolean;
  readonly lastArtifactName: string | null;
  readonly error: string | null;
};

export class RecordingViewModelBuilder {
  static build(input: RecordingViewModelInput): RecordingViewModel {
    const handle = input.liveConnection?.handle;
    const supported = Boolean(handle?.startRecording && handle.stopRecording);
    const connected = Boolean(input.liveConnection);
    return {
      status: input.recording ? 'recording' : 'idle',
      sessionLabel: input.liveConnection?.sessionId ?? '--',
      artifactLabel: input.lastArtifactName ?? '--',
      note: supported
        ? 'Records the composed browser view as a local WebM artifact.'
        : 'Connect to a session with recording-capable browser handle support.',
      canStart: connected && supported && !input.recording && !input.busy,
      canStop: connected && supported && input.recording && !input.busy,
      canDownload: Boolean(input.lastArtifactName) && !input.busy,
      busy: input.busy,
      error: input.error,
    };
  }
}
