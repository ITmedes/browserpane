import { CameraProfileCatalog, type CameraProfile } from './camera-profile-catalog.js';
import { CameraTelemetryTracker } from './camera-telemetry-tracker.js';

export type CameraSendResult = 'sent' | 'queued' | 'replaced';
export type SendCameraFrameFn = (payload: Uint8Array) => CameraSendResult;

const CAMERA_ENCODER_QUEUE_LIMIT = 2;

type CameraCaptureRuntimeInput = {
  videoElement: HTMLVideoElement;
  canvasElement: HTMLCanvasElement;
  canvasContext: CanvasRenderingContext2D;
  sendFrame: SendCameraFrameFn;
  telemetry: CameraTelemetryTracker;
  onEncoderError: (error: DOMException) => void;
};

export class CameraCaptureRuntime {
  private readonly videoElement: HTMLVideoElement;
  private readonly canvasElement: HTMLCanvasElement;
  private readonly canvasContext: CanvasRenderingContext2D;
  private readonly sendFrame: SendCameraFrameFn;
  private readonly telemetry: CameraTelemetryTracker;
  private readonly onEncoderError: (error: DOMException) => void;

  private encoder: VideoEncoder | null = null;
  private captureTimer: ReturnType<typeof setInterval> | null = null;
  private activeProfile: CameraProfile | null = null;
  private capturePending = false;
  private forceKeyframe = true;
  private frameTimestampUs = 0;
  private encodeStarts = new Map<number, number>();

  constructor(input: CameraCaptureRuntimeInput) {
    this.videoElement = input.videoElement;
    this.canvasElement = input.canvasElement;
    this.canvasContext = input.canvasContext;
    this.sendFrame = input.sendFrame;
    this.telemetry = input.telemetry;
    this.onEncoderError = input.onEncoderError;
  }

  applyProfile(profile: CameraProfile): void {
    this.activeProfile = profile;
    this.resetSequencing();
    this.canvasElement.width = profile.width;
    this.canvasElement.height = profile.height;

    if (this.encoder) {
      try {
        this.encoder.close();
      } catch (_) {
        // Ignore encoder close failures during reconfiguration.
      }
    }

    this.encoder = new VideoEncoder({
      output: (chunk) => this.handleEncodedChunk(chunk),
      error: (error) => this.onEncoderError(error),
    });
    this.encoder.configure(CameraProfileCatalog.toEncoderConfig(profile));
    this.restartCaptureTimer();
  }

  stop(): void {
    if (this.captureTimer) {
      clearInterval(this.captureTimer);
      this.captureTimer = null;
    }

    this.activeProfile = null;
    this.resetSequencing();

    if (this.encoder) {
      try {
        this.encoder.close();
      } catch (_) {
        // Ignore encoder close failures during teardown.
      }
      this.encoder = null;
    }
  }

  private resetSequencing(): void {
    this.capturePending = false;
    this.forceKeyframe = true;
    this.frameTimestampUs = 0;
    this.encodeStarts.clear();
  }

  private restartCaptureTimer(): void {
    if (this.captureTimer) {
      clearInterval(this.captureTimer);
      this.captureTimer = null;
    }

    if (!this.activeProfile) {
      return;
    }

    const intervalMs = Math.max(16, Math.round(1000 / this.activeProfile.fps));
    this.captureTimer = setInterval(() => {
      void this.captureFrame();
    }, intervalMs);
  }

  private handleEncodedChunk(chunk: EncodedVideoChunk): void {
    let encodeTimeMs: number | undefined;
    const startedAt = this.encodeStarts.get(chunk.timestamp);
    if (typeof startedAt === 'number') {
      this.encodeStarts.delete(chunk.timestamp);
      encodeTimeMs = performance.now() - startedAt;
    }

    const payload = new Uint8Array(chunk.byteLength);
    chunk.copyTo(payload);
    const sendResult = this.sendFrame(payload);

    this.telemetry.recordEncodedChunk({
      chunkType: chunk.type,
      chunkByteLength: payload.byteLength,
      sendResult,
      encodeTimeMs,
    });
  }

  private async captureFrame(): Promise<void> {
    if (!this.activeProfile || !this.encoder || this.capturePending) {
      return;
    }

    if (this.videoElement.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) {
      return;
    }

    if (this.encoder.encodeQueueSize > CAMERA_ENCODER_QUEUE_LIMIT) {
      this.telemetry.recordEncoderQueueDrop();
      return;
    }

    this.capturePending = true;
    try {
      this.canvasContext.drawImage(
        this.videoElement,
        0,
        0,
        this.canvasElement.width,
        this.canvasElement.height,
      );
      this.telemetry.recordCapture();

      const timestampUs = this.frameTimestampUs;
      this.frameTimestampUs += Math.round(1_000_000 / this.activeProfile.fps);
      const encodedFrames = this.telemetry.getFramesEncoded();
      const keyFrame = this.forceKeyframe
        || (encodedFrames > 0 && (encodedFrames % this.activeProfile.keyframeInterval) === 0);
      this.forceKeyframe = false;

      const frame = new VideoFrame(this.canvasElement, { timestamp: timestampUs });
      this.encodeStarts.set(timestampUs, performance.now());
      try {
        this.encoder.encode(frame, { keyFrame });
      } finally {
        frame.close();
      }
    } catch (error) {
      console.error('[bpane] camera frame capture failed:', error);
    } finally {
      this.capturePending = false;
    }
  }
}
