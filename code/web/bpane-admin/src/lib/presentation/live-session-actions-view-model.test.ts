import { describe, expect, it } from 'vitest';
import { LiveSessionActionsViewModelBuilder } from './live-session-actions-view-model';

describe('LiveSessionActionsViewModelBuilder', () => {
  it('disables live actions before a browser connection', () => {
    const viewModel = LiveSessionActionsViewModelBuilder.build({
      connected: false,
      cameraAvailable: true,
      microphoneAvailable: true,
      uploadAvailable: true,
      cameraActive: false,
      microphoneActive: false,
      busyAction: null,
      error: null,
    });

    expect(viewModel.cameraDisabled).toBe(true);
    expect(viewModel.microphoneDisabled).toBe(true);
    expect(viewModel.uploadDisabled).toBe(true);
  });

  it('labels and enables supported live actions while connected', () => {
    const viewModel = LiveSessionActionsViewModelBuilder.build({
      connected: true,
      cameraAvailable: true,
      microphoneAvailable: true,
      uploadAvailable: true,
      cameraActive: true,
      microphoneActive: false,
      busyAction: null,
      error: null,
    });

    expect(viewModel.cameraLabel).toBe('Stop camera');
    expect(viewModel.microphoneLabel).toBe('Start microphone');
    expect(viewModel.uploadDisabled).toBe(false);
  });

  it('disables all actions while one action is busy', () => {
    const viewModel = LiveSessionActionsViewModelBuilder.build({
      connected: true,
      cameraAvailable: true,
      microphoneAvailable: true,
      uploadAvailable: true,
      cameraActive: false,
      microphoneActive: false,
      busyAction: 'upload',
      error: 'upload failed',
    });

    expect(viewModel.busy).toBe(true);
    expect(viewModel.cameraDisabled).toBe(true);
    expect(viewModel.error).toBe('upload failed');
  });
});
