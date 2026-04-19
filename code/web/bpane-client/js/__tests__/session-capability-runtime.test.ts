import { describe, expect, it, vi } from 'vitest';
import { SessionCapabilityRuntime } from '../session-capability-runtime.js';

function createRuntime(options: {
  fileTransferOptionEnabled?: boolean;
} = {}) {
  const stopMicrophone = vi.fn();
  const stopCamera = vi.fn();
  const setFileTransferEnabled = vi.fn();
  const onCapabilitiesChange = vi.fn();
  const runtime = new SessionCapabilityRuntime({
    fileTransferOptionEnabled: options.fileTransferOptionEnabled ?? true,
    stopMicrophone,
    stopCamera,
    setFileTransferEnabled,
    onCapabilitiesChange,
  });

  return {
    runtime,
    stopMicrophone,
    stopCamera,
    setFileTransferEnabled,
    onCapabilitiesChange,
  };
}

describe('SessionCapabilityRuntime', () => {
  it('enables supported owner capabilities from session flags', () => {
    const {
      runtime,
      stopMicrophone,
      stopCamera,
      setFileTransferEnabled,
      onCapabilitiesChange,
    } = createRuntime();

    const next = runtime.apply({
      current: {
        audio: false,
        microphone: false,
        camera: false,
        fileTransfer: false,
        keyboardLayout: false,
      },
      sessionFlags: 0x1d,
      microphoneEncoderSupported: true,
      cameraEncoderSupported: true,
      resolutionLocked: false,
    });

    expect(next).toEqual({
      audio: true,
      microphone: true,
      camera: true,
      fileTransfer: true,
      keyboardLayout: false,
    });
    expect(stopMicrophone).not.toHaveBeenCalled();
    expect(stopCamera).not.toHaveBeenCalled();
    expect(setFileTransferEnabled).toHaveBeenCalledWith(true);
    expect(onCapabilitiesChange).toHaveBeenCalledWith(next);
  });

  it('suppresses viewer-only and unsupported capabilities and stops disabled media', () => {
    const {
      runtime,
      stopMicrophone,
      stopCamera,
      setFileTransferEnabled,
      onCapabilitiesChange,
    } = createRuntime();

    const next = runtime.apply({
      current: {
        audio: true,
        microphone: true,
        camera: true,
        fileTransfer: true,
        keyboardLayout: true,
      },
      sessionFlags: 0x3d,
      microphoneEncoderSupported: false,
      cameraEncoderSupported: true,
      resolutionLocked: true,
    });

    expect(next).toEqual({
      audio: true,
      microphone: false,
      camera: false,
      fileTransfer: false,
      keyboardLayout: false,
    });
    expect(stopMicrophone).toHaveBeenCalledTimes(1);
    expect(stopCamera).toHaveBeenCalledTimes(1);
    expect(setFileTransferEnabled).toHaveBeenCalledWith(false);
    expect(onCapabilitiesChange).toHaveBeenCalledWith(next);
  });

  it('respects the file transfer option override and skips duplicate change notifications', () => {
    const {
      runtime,
      stopMicrophone,
      stopCamera,
      setFileTransferEnabled,
      onCapabilitiesChange,
    } = createRuntime({
      fileTransferOptionEnabled: false,
    });

    const current = {
      audio: true,
      microphone: false,
      camera: false,
      fileTransfer: true,
      keyboardLayout: false,
    };

    const next = runtime.apply({
      current,
      sessionFlags: 0x05,
      microphoneEncoderSupported: true,
      cameraEncoderSupported: true,
      resolutionLocked: false,
    });

    expect(next).toEqual({
      audio: true,
      microphone: false,
      camera: false,
      fileTransfer: true,
      keyboardLayout: false,
    });
    expect(stopMicrophone).toHaveBeenCalledTimes(1);
    expect(stopCamera).toHaveBeenCalledTimes(1);
    expect(setFileTransferEnabled).toHaveBeenCalledWith(false);
    expect(onCapabilitiesChange).not.toHaveBeenCalled();
  });
});
