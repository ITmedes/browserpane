export type LiveSessionActionId = 'camera' | 'microphone' | 'upload';

export type LiveSessionActionsViewModel = {
  readonly cameraLabel: string;
  readonly microphoneLabel: string;
  readonly cameraDisabled: boolean;
  readonly microphoneDisabled: boolean;
  readonly uploadDisabled: boolean;
  readonly busy: boolean;
  readonly error: string | null;
};

export type LiveSessionActionsViewModelInput = {
  readonly connected: boolean;
  readonly cameraAvailable: boolean;
  readonly microphoneAvailable: boolean;
  readonly uploadAvailable: boolean;
  readonly cameraActive: boolean;
  readonly microphoneActive: boolean;
  readonly busyAction: LiveSessionActionId | null;
  readonly error: string | null;
};

export class LiveSessionActionsViewModelBuilder {
  static build(input: LiveSessionActionsViewModelInput): LiveSessionActionsViewModel {
    const liveDisabled = !input.connected || input.busyAction !== null;
    return {
      cameraLabel: input.cameraActive ? 'Stop camera' : 'Start camera',
      microphoneLabel: input.microphoneActive ? 'Stop microphone' : 'Start microphone',
      cameraDisabled: liveDisabled || !input.cameraAvailable,
      microphoneDisabled: liveDisabled || !input.microphoneAvailable,
      uploadDisabled: liveDisabled || !input.uploadAvailable,
      busy: input.busyAction !== null,
      error: input.error,
    };
  }
}
