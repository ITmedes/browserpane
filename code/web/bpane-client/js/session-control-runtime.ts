export interface SessionControlRuntimeInput {
  setRemoteSize: (width: number, height: number) => void;
  onResolutionChange?: (width: number, height: number) => void;
  setSessionFlags: (flags: number) => void;
  setMicrophoneSupported: (supported: boolean) => void;
  setCameraSupported: (supported: boolean) => void;
  configureInputExtendedKeyEvents: (enabled: boolean) => void;
  sendLayoutHint: () => void;
  updateCapabilities: () => void;
  applyClientAccessState: (flags: number, width: number, height: number) => void;
  sendControlFrame: (payload: Uint8Array) => void;
}

export class SessionControlRuntime {
  private readonly setRemoteSize: (width: number, height: number) => void;
  private readonly onResolutionChange?: (width: number, height: number) => void;
  private readonly setSessionFlags: (flags: number) => void;
  private readonly setMicrophoneSupported: (supported: boolean) => void;
  private readonly setCameraSupported: (supported: boolean) => void;
  private readonly configureInputExtendedKeyEvents: (enabled: boolean) => void;
  private readonly sendLayoutHint: () => void;
  private readonly updateCapabilities: () => void;
  private readonly applyClientAccessState: (flags: number, width: number, height: number) => void;
  private readonly sendControlFrame: (payload: Uint8Array) => void;

  constructor(input: SessionControlRuntimeInput) {
    this.setRemoteSize = input.setRemoteSize;
    this.onResolutionChange = input.onResolutionChange;
    this.setSessionFlags = input.setSessionFlags;
    this.setMicrophoneSupported = input.setMicrophoneSupported;
    this.setCameraSupported = input.setCameraSupported;
    this.configureInputExtendedKeyEvents = input.configureInputExtendedKeyEvents;
    this.sendLayoutHint = input.sendLayoutHint;
    this.updateCapabilities = input.updateCapabilities;
    this.applyClientAccessState = input.applyClientAccessState;
    this.sendControlFrame = input.sendControlFrame;
  }

  handle(payload: Uint8Array): void {
    if (payload.length < 1) {
      return;
    }

    switch (payload[0]) {
      case 0x02:
        this.handleResolutionAck(payload);
        break;
      case 0x03:
        this.handleSessionReady(payload);
        break;
      case 0x04:
        this.handlePing(payload);
        break;
      case 0x08:
        this.handleLegacyResolutionLock(payload);
        break;
      case 0x09:
        this.handleClientAccessState(payload);
        break;
      default:
        break;
    }
  }

  private handleResolutionAck(payload: Uint8Array): void {
    if (payload.length < 5) {
      return;
    }
    const width = payload[1] | (payload[2] << 8);
    const height = payload[3] | (payload[4] << 8);
    this.setRemoteSize(width, height);
    this.onResolutionChange?.(width, height);
  }

  private handleSessionReady(payload: Uint8Array): void {
    if (payload.length < 3) {
      return;
    }
    const flags = payload[2];
    this.setSessionFlags(flags);
    const supportsEx = (flags & 0x20) !== 0;
    this.setMicrophoneSupported((flags & 0x08) !== 0);
    this.setCameraSupported((flags & 0x10) !== 0);
    this.configureInputExtendedKeyEvents(supportsEx);
    if (supportsEx) {
      this.sendLayoutHint();
    }
    this.updateCapabilities();
  }

  private handlePing(payload: Uint8Array): void {
    if (payload.length < 13) {
      return;
    }
    const pong = new Uint8Array(13);
    pong[0] = 0x05;
    pong.set(payload.slice(1, 13), 1);
    this.sendControlFrame(pong);
  }

  private handleLegacyResolutionLock(payload: Uint8Array): void {
    if (payload.length < 5) {
      return;
    }
    const width = payload[1] | (payload[2] << 8);
    const height = payload[3] | (payload[4] << 8);
    this.applyClientAccessState(0x03, width, height);
  }

  private handleClientAccessState(payload: Uint8Array): void {
    if (payload.length < 6) {
      return;
    }
    const flags = payload[1];
    const width = payload[2] | (payload[3] << 8);
    const height = payload[4] | (payload[5] << 8);
    this.applyClientAccessState(flags, width, height);
  }
}
