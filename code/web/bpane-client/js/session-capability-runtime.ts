import type { SessionCapabilities } from './bpane.js';

export interface SessionCapabilityRuntimeInput {
  fileTransferOptionEnabled: boolean;
  stopMicrophone: () => void;
  stopCamera: () => void;
  setFileTransferEnabled: (enabled: boolean) => void;
  onCapabilitiesChange?: (capabilities: SessionCapabilities) => void;
}

export interface SessionCapabilityStateInput {
  current: SessionCapabilities;
  sessionFlags: number;
  microphoneEncoderSupported: boolean;
  cameraEncoderSupported: boolean;
  resolutionLocked: boolean;
}

export class SessionCapabilityRuntime {
  private readonly fileTransferOptionEnabled: boolean;
  private readonly stopMicrophone: () => void;
  private readonly stopCamera: () => void;
  private readonly setFileTransferEnabled: (enabled: boolean) => void;
  private readonly onCapabilitiesChange?: (capabilities: SessionCapabilities) => void;

  constructor(input: SessionCapabilityRuntimeInput) {
    this.fileTransferOptionEnabled = input.fileTransferOptionEnabled;
    this.stopMicrophone = input.stopMicrophone;
    this.stopCamera = input.stopCamera;
    this.setFileTransferEnabled = input.setFileTransferEnabled;
    this.onCapabilitiesChange = input.onCapabilitiesChange;
  }

  apply(input: SessionCapabilityStateInput): SessionCapabilities {
    const next = this.resolveNext(input);

    if (!next.microphone) {
      this.stopMicrophone();
    }
    if (!next.camera) {
      this.stopCamera();
    }
    this.setFileTransferEnabled(this.fileTransferOptionEnabled && next.fileTransfer);

    if (this.hasChanged(input.current, next)) {
      this.onCapabilitiesChange?.({ ...next });
    }

    return next;
  }

  private resolveNext(input: SessionCapabilityStateInput): SessionCapabilities {
    return {
      audio: (input.sessionFlags & 0x01) !== 0,
      microphone: (input.sessionFlags & 0x08) !== 0
        && input.microphoneEncoderSupported
        && !input.resolutionLocked,
      camera: (input.sessionFlags & 0x10) !== 0
        && input.cameraEncoderSupported
        && !input.resolutionLocked,
      fileTransfer: (input.sessionFlags & 0x04) !== 0 && !input.resolutionLocked,
      keyboardLayout: (input.sessionFlags & 0x20) !== 0 && !input.resolutionLocked,
    };
  }

  private hasChanged(current: SessionCapabilities, next: SessionCapabilities): boolean {
    return current.audio !== next.audio
      || current.microphone !== next.microphone
      || current.camera !== next.camera
      || current.fileTransfer !== next.fileTransfer
      || current.keyboardLayout !== next.keyboardLayout;
  }
}
