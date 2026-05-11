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
    });

    expect(viewModel.renderBackend).toBe('webgl2');
    expect(viewModel.hiDpiEnabled).toBe(false);
    expect(viewModel.reconnectHint).toContain('next connect');
  });

  it('exposes render preferences while connected', () => {
    const viewModel = DisplaySettingsViewModelBuilder.build({
      preferences: DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES,
      connected: true,
    });

    expect(viewModel.connectionLabel).toBe('Live browser connected');
    expect(viewModel.reconnectHint).toContain('after reconnect');
    expect(viewModel.scrollCopyEnabled).toBe(true);
  });

  it('exposes the supported render backend order', () => {
    expect(DISPLAY_BACKEND_OPTIONS.map((option) => option.value)).toEqual([
      'auto',
      'webgl2',
      'canvas2d',
    ]);
  });
});
