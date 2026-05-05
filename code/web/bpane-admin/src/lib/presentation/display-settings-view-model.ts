import type {
  BrowserSessionConnectPreferences,
  BrowserSessionRenderBackend,
} from '../session/browser-session-types';

export type DisplayBackendOptionViewModel = {
  readonly value: BrowserSessionRenderBackend;
  readonly label: string;
};

export type DisplaySettingsViewModel = {
  readonly connectionLabel: string;
  readonly reconnectHint: string;
  readonly renderBackend: BrowserSessionRenderBackend;
  readonly hiDpiEnabled: boolean;
  readonly scrollCopyEnabled: boolean;
  readonly microphoneLabel: string;
  readonly cameraLabel: string;
  readonly microphoneDisabled: boolean;
  readonly cameraDisabled: boolean;
  readonly uploadDisabled: boolean;
  readonly busy: boolean;
  readonly error: string | null;
};

export type DisplaySettingsViewModelInput = {
  readonly preferences: BrowserSessionConnectPreferences;
  readonly connected: boolean;
  readonly microphoneAvailable: boolean;
  readonly cameraAvailable: boolean;
  readonly uploadAvailable: boolean;
  readonly microphoneActive: boolean;
  readonly cameraActive: boolean;
  readonly busy: boolean;
  readonly error: string | null;
};

export const DISPLAY_BACKEND_OPTIONS = Object.freeze([
  { value: 'auto', label: 'Auto' },
  { value: 'webgl2', label: 'WebGL 2' },
  { value: 'canvas2d', label: 'Canvas 2D' },
] satisfies readonly DisplayBackendOptionViewModel[]);

export class DisplaySettingsViewModelBuilder {
  static build(input: DisplaySettingsViewModelInput): DisplaySettingsViewModel {
    const liveDisabled = !input.connected || input.busy;
    return {
      connectionLabel: input.connected ? 'Live browser connected' : 'Connect the browser to use live controls',
      reconnectHint: input.connected ? 'Render changes apply after reconnect.' : 'Render changes apply to the next connect.',
      renderBackend: input.preferences.renderBackend,
      hiDpiEnabled: input.preferences.hiDpi,
      scrollCopyEnabled: input.preferences.scrollCopy,
      microphoneLabel: input.microphoneActive ? 'Stop microphone' : 'Start microphone',
      cameraLabel: input.cameraActive ? 'Stop camera' : 'Start camera',
      microphoneDisabled: liveDisabled || !input.microphoneAvailable,
      cameraDisabled: liveDisabled || !input.cameraAvailable,
      uploadDisabled: liveDisabled || !input.uploadAvailable,
      busy: input.busy,
      error: input.error,
    };
  }
}
