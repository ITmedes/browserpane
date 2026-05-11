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
};

export type DisplaySettingsViewModelInput = {
  readonly preferences: BrowserSessionConnectPreferences;
  readonly connected: boolean;
};

export const DISPLAY_BACKEND_OPTIONS = Object.freeze([
  { value: 'auto', label: 'Auto' },
  { value: 'webgl2', label: 'WebGL 2' },
  { value: 'canvas2d', label: 'Canvas 2D' },
] satisfies readonly DisplayBackendOptionViewModel[]);

export class DisplaySettingsViewModelBuilder {
  static build(input: DisplaySettingsViewModelInput): DisplaySettingsViewModel {
    return {
      connectionLabel: input.connected ? 'Live browser connected' : 'No live browser connected',
      reconnectHint: input.connected ? 'Render changes apply after reconnect.' : 'Render changes apply to the next connect.',
      renderBackend: input.preferences.renderBackend,
      hiDpiEnabled: input.preferences.hiDpi,
      scrollCopyEnabled: input.preferences.scrollCopy,
    };
  }
}
