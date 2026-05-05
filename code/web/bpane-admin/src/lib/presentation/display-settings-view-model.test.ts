import { describe, expect, it } from 'vitest';
import { DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES } from '../session/browser-session-types';
import { DISPLAY_BACKEND_OPTIONS, DisplaySettingsViewModelBuilder } from './display-settings-view-model';

describe('DisplaySettingsViewModelBuilder', () => {
  it('keeps render preferences available before a browser connection', () => {
    const viewModel = DisplaySettingsViewModelBuilder.build({
      preferences: {
        ...DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES,
        renderBackend: 'webgl2',
        hiDpi: false,
      },
      connected: false,
      microphoneAvailable: false,
      cameraAvailable: false,
      uploadAvailable: false,
      microphoneActive: false,
      cameraActive: false,
      busy: false,
      error: null,
    });

    expect(viewModel.renderBackend).toBe('webgl2');
    expect(viewModel.hiDpiEnabled).toBe(false);
    expect(viewModel.microphoneDisabled).toBe(true);
    expect(viewModel.uploadDisabled).toBe(true);
    expect(viewModel.reconnectHint).toContain('next connect');
  });

  it('enables live media actions only when the connected handle supports them', () => {
    const viewModel = DisplaySettingsViewModelBuilder.build({
      preferences: DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES,
      connected: true,
      microphoneAvailable: true,
      cameraAvailable: false,
      uploadAvailable: true,
      microphoneActive: true,
      cameraActive: false,
      busy: false,
      error: 'media permission denied',
    });

    expect(viewModel.microphoneLabel).toBe('Stop microphone');
    expect(viewModel.microphoneDisabled).toBe(false);
    expect(viewModel.cameraDisabled).toBe(true);
    expect(viewModel.uploadDisabled).toBe(false);
    expect(viewModel.error).toBe('media permission denied');
  });

  it('exposes the supported render backend order', () => {
    expect(DISPLAY_BACKEND_OPTIONS.map((option) => option.value)).toEqual([
      'auto',
      'webgl2',
      'canvas2d',
    ]);
  });
});
