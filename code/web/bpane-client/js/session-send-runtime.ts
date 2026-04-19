import {
  CH_AUDIO_IN,
  CH_CLIPBOARD,
  CH_CONTROL,
  CH_FILE_UP,
  CH_INPUT,
  CH_VIDEO_IN,
  CTRL_KEYBOARD_LAYOUT,
  encodeFrame,
  INPUT_MOUSE_MOVE,
} from './protocol.js';

export interface SessionSendRuntimeInput {
  isViewerRestricted: () => boolean;
  recordTx: (channelId: number, bytes: number) => void;
  onWriteError?: (message: string, error: unknown) => void;
  setTimeoutFn?: Window['setTimeout'];
  clearTimeoutFn?: Window['clearTimeout'];
}

export class SessionSendRuntime {
  private readonly isViewerRestricted: () => boolean;
  private readonly recordTx: (channelId: number, bytes: number) => void;
  private readonly onWriteError?: (message: string, error: unknown) => void;
  private readonly setTimeoutFn: Window['setTimeout'];
  private readonly clearTimeoutFn: Window['clearTimeout'];

  private sendWriter: WritableStreamDefaultWriter<Uint8Array> | null = null;
  private pendingFrames: Uint8Array[] = [];
  private pendingCameraFrame: Uint8Array | null = null;
  private pendingCameraFlushTimer: number | null = null;

  constructor(input: SessionSendRuntimeInput) {
    this.isViewerRestricted = input.isViewerRestricted;
    this.recordTx = input.recordTx;
    this.onWriteError = input.onWriteError;
    this.setTimeoutFn = input.setTimeoutFn ?? window.setTimeout.bind(window);
    this.clearTimeoutFn = input.clearTimeoutFn ?? window.clearTimeout.bind(window);
  }

  hasWriter(): boolean {
    return this.sendWriter !== null;
  }

  attachWriter(
    writer: WritableStreamDefaultWriter<Uint8Array>,
    onAttached?: () => void,
  ): void {
    this.sendWriter = writer;
    if (this.pendingFrames.length > 0) {
      const queued = this.pendingFrames.splice(0);
      queued.forEach((frame) => {
        this.recordTx(frame[0] ?? 0, frame.length);
        this.sendWriter?.write(frame).catch((error) => {
          this.onWriteError?.('[bpane] sendFrame write failed', error);
        });
      });
    }
    this.schedulePendingCameraFlush(0);
    onAttached?.();
  }

  destroy(): void {
    if (this.pendingCameraFlushTimer !== null) {
      this.clearTimeoutFn(this.pendingCameraFlushTimer);
      this.pendingCameraFlushTimer = null;
    }
    this.sendWriter = null;
    this.pendingFrames = [];
    this.pendingCameraFrame = null;
  }

  sendFrame(channelId: number, payload: Uint8Array): void {
    if (this.isViewerBlockedChannel(channelId, payload)) {
      return;
    }

    const frame = encodeFrame(channelId, payload);

    if (!this.sendWriter) {
      this.pendingFrames.push(frame);
      return;
    }

    const desiredSize = this.sendWriter.desiredSize;
    if (desiredSize !== null && desiredSize <= 0 && channelId === CH_INPUT && payload[0] === INPUT_MOUSE_MOVE) {
      return;
    }
    if (desiredSize !== null && desiredSize <= 0 && channelId === CH_VIDEO_IN && payload.length > 0) {
      return;
    }

    this.recordTx(channelId, frame.length);
    this.sendWriter.write(frame).catch((error) => {
      this.onWriteError?.('[bpane] sendFrame write failed', error);
    });
  }

  sendCameraFrame(payload: Uint8Array): 'sent' | 'queued' | 'replaced' {
    if (payload.length === 0) {
      this.pendingCameraFrame = null;
      this.sendFrame(CH_VIDEO_IN, payload);
      return 'sent';
    }

    const frame = encodeFrame(CH_VIDEO_IN, payload);
    if (!this.sendWriter) {
      const replaced = this.pendingCameraFrame !== null;
      this.pendingCameraFrame = frame;
      return replaced ? 'replaced' : 'queued';
    }

    const desiredSize = this.sendWriter.desiredSize;
    if (desiredSize !== null && desiredSize <= 0) {
      const replaced = this.pendingCameraFrame !== null;
      this.pendingCameraFrame = frame;
      this.schedulePendingCameraFlush();
      return replaced ? 'replaced' : 'queued';
    }

    this.recordTx(CH_VIDEO_IN, frame.length);
    this.sendWriter.write(frame).catch((error) => {
      this.onWriteError?.('[bpane] camera frame write failed', error);
    });
    return 'sent';
  }

  private schedulePendingCameraFlush(delayMs = 40): void {
    if (this.pendingCameraFlushTimer !== null) {
      return;
    }
    this.pendingCameraFlushTimer = this.setTimeoutFn(() => {
      this.pendingCameraFlushTimer = null;
      this.flushPendingCameraFrame();
    }, delayMs);
  }

  private flushPendingCameraFrame(): void {
    if (!this.pendingCameraFrame || !this.sendWriter) {
      return;
    }

    const desiredSize = this.sendWriter.desiredSize;
    if (desiredSize !== null && desiredSize <= 0) {
      this.schedulePendingCameraFlush();
      return;
    }

    const frame = this.pendingCameraFrame;
    this.pendingCameraFrame = null;
    this.recordTx(CH_VIDEO_IN, frame.length);
    this.sendWriter.write(frame).catch((error) => {
      this.onWriteError?.('[bpane] pending camera frame write failed', error);
    });
    if (this.pendingCameraFrame) {
      this.schedulePendingCameraFlush(0);
    }
  }

  private isViewerBlockedChannel(channelId: number, payload: Uint8Array): boolean {
    if (!this.isViewerRestricted()) {
      return false;
    }

    if (channelId === CH_CONTROL) {
      return payload[0] === CTRL_KEYBOARD_LAYOUT;
    }

    return channelId === CH_INPUT
      || channelId === CH_CLIPBOARD
      || channelId === CH_AUDIO_IN
      || channelId === CH_VIDEO_IN
      || channelId === CH_FILE_UP;
  }
}
