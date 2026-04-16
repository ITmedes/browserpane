import {
  evaluateCameraAdaptation,
  type QualityLimitationReason,
} from './camera/camera-adaptation-policy.js';
import {
  CameraProfileCatalog,
  type CameraProfile,
} from './camera/camera-profile-catalog.js';
import { CameraTelemetryTracker } from './camera/camera-telemetry-tracker.js';

type CameraSendResult = 'sent' | 'queued' | 'replaced';
type SendCameraFrameFn = (payload: Uint8Array) => CameraSendResult;

export interface CameraTelemetryProfile extends Pick<
  CameraProfile,
  'name' | 'width' | 'height' | 'fps' | 'bitrate' | 'smooth' | 'powerEfficient'
> {}

export interface CameraTelemetrySnapshot {
  supported: boolean;
  active: boolean;
  profile: CameraTelemetryProfile | null;
  qualityLimitationReason: QualityLimitationReason;
  framesCaptured: number;
  framesEncoded: number;
  keyframesEncoded: number;
  encodedBytes: number;
  transportFramesQueued: number;
  transportFramesReplaced: number;
  encoderQueueDrops: number;
  averageEncodeTimeMs: number;
  maxEncodeTimeMs: number;
  profileUpgrades: number;
  profileDowngrades: number;
  reconfigurations: number;
}

const CAMERA_ADAPT_INTERVAL_MS = 2000;
const CAMERA_ENCODER_QUEUE_LIMIT = 2;

export class CameraController {
  private sendFrame: SendCameraFrameFn;
  private stream: MediaStream | null = null;
  private videoEl: HTMLVideoElement | null = null;
  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private encoder: VideoEncoder | null = null;
  private captureTimer: ReturnType<typeof setInterval> | null = null;
  private adaptationTimer: ReturnType<typeof setInterval> | null = null;
  private active = false;
  private capturePending = false;
  private activeProfileIndex = -1;
  private supportedProfiles: CameraProfile[] = [];
  private forceKeyframe = true;
  private frameTimestampUs = 0;
  private encodeStarts = new Map<number, number>();
  private qualityLimitationReason: QualityLimitationReason = 'none';
  private stableWindows = 0;
  private telemetry = new CameraTelemetryTracker();

  constructor(sendFrame: SendCameraFrameFn) {
    this.sendFrame = sendFrame;
  }

  static async isSupported(): Promise<boolean> {
    const supported = await CameraProfileCatalog.getSupportedProfiles();
    return supported.length > 0;
  }

  static async getSupportedProfiles(): Promise<CameraProfile[]> {
    return CameraProfileCatalog.getSupportedProfiles();
  }

  async startCamera(): Promise<void> {
    if (this.active) return;
    try {
      this.supportedProfiles = await CameraProfileCatalog.getSupportedProfiles();
      const initialProfile = this.supportedProfiles[0];
      if (!initialProfile) {
        throw new Error('camera video encoding is not supported in this browser');
      }

      this.stream = await navigator.mediaDevices.getUserMedia(CameraProfileCatalog.getCaptureConstraints());

      this.videoEl = document.createElement('video');
      this.videoEl.muted = true;
      this.videoEl.playsInline = true;
      this.videoEl.autoplay = true;
      this.videoEl.srcObject = this.stream;

      this.canvas = document.createElement('canvas');
      this.ctx = this.canvas.getContext('2d');
      if (!this.ctx) {
        throw new Error('camera canvas context unavailable');
      }

      await this.videoEl.play();

      this.resetTelemetry();
      this.active = true;
      this.applyProfile(0, 'none');
      this.adaptationTimer = setInterval(() => {
        this.evaluateAdaptation();
      }, CAMERA_ADAPT_INTERVAL_MS);
    } catch (e) {
      console.error('[bpane] camera error:', e);
      this.stopCamera();
      throw e instanceof Error ? e : new Error(String(e));
    }
  }

  stopCamera(): void {
    if (!this.active && !this.stream) return;
    this.active = false;
    if (this.captureTimer) {
      clearInterval(this.captureTimer);
      this.captureTimer = null;
    }
    if (this.adaptationTimer) {
      clearInterval(this.adaptationTimer);
      this.adaptationTimer = null;
    }
    this.capturePending = false;
    this.encodeStarts.clear();
    if (this.encoder) {
      try { this.encoder.close(); } catch (_) { /* ignore */ }
      this.encoder = null;
    }
    if (this.stream) {
      this.stream.getTracks().forEach((track) => track.stop());
      this.stream = null;
    }
    if (this.videoEl) {
      this.videoEl.pause();
      this.videoEl.srcObject = null;
      this.videoEl = null;
    }
    this.canvas = null;
    this.ctx = null;
    this.activeProfileIndex = -1;
    this.frameTimestampUs = 0;
    this.forceKeyframe = true;
    this.sendFrame(new Uint8Array());
  }

  destroy(): void {
    this.stopCamera();
  }

  getStats(): CameraTelemetrySnapshot {
    const profile = this.activeProfileIndex >= 0 ? { ...this.supportedProfiles[this.activeProfileIndex] } : null;
    const metrics = this.telemetry.getMetrics();
    return {
      supported: this.supportedProfiles.length > 0,
      active: this.active,
      profile,
      qualityLimitationReason: this.qualityLimitationReason,
      ...metrics,
    };
  }

  private resetTelemetry(): void {
    this.qualityLimitationReason = 'none';
    this.stableWindows = 0;
    this.frameTimestampUs = 0;
    this.forceKeyframe = true;
    this.telemetry.reset();
    this.resetAdaptationWindow();
  }

  private resetAdaptationWindow(): void {
    this.telemetry.resetWindow();
  }

  private applyProfile(index: number, reason: QualityLimitationReason): void {
    const profile = this.supportedProfiles[index];
    if (!profile) return;

    const previousIndex = this.activeProfileIndex;
    this.telemetry.recordProfileChange(previousIndex, index);

    this.activeProfileIndex = index;
    this.qualityLimitationReason = reason;
    this.forceKeyframe = true;
    this.frameTimestampUs = 0;
    this.encodeStarts.clear();

    if (this.canvas) {
      this.canvas.width = profile.width;
      this.canvas.height = profile.height;
    }

    if (this.encoder) {
      try { this.encoder.close(); } catch (_) { /* ignore */ }
    }
    this.encoder = new VideoEncoder({
      output: (chunk) => this.handleEncodedChunk(chunk),
      error: (e) => {
        console.error('[bpane] camera encoder error:', e);
        this.stopCamera();
      },
    });
    this.encoder.configure(CameraProfileCatalog.toEncoderConfig(profile));
    this.restartCaptureTimer();
  }

  private restartCaptureTimer(): void {
    if (this.captureTimer) {
      clearInterval(this.captureTimer);
      this.captureTimer = null;
    }
    const profile = this.supportedProfiles[this.activeProfileIndex];
    if (!this.active || !profile) return;
    const intervalMs = Math.max(16, Math.round(1000 / profile.fps));
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

    const result = this.sendFrame(payload);
    this.telemetry.recordEncodedChunk({
      chunkType: chunk.type,
      chunkByteLength: payload.byteLength,
      sendResult: result,
      encodeTimeMs,
    });
  }

  private evaluateAdaptation(): void {
    const profile = this.supportedProfiles[this.activeProfileIndex];
    if (!this.active || !profile) return;
    const decision = evaluateCameraAdaptation({
      activeProfileIndex: this.activeProfileIndex,
      profileCount: this.supportedProfiles.length,
      currentProfileFps: profile.fps,
      stableWindows: this.stableWindows,
      qualityLimitationReason: this.qualityLimitationReason,
      windowStats: this.telemetry.getWindowStats(),
    });

    this.stableWindows = decision.nextStableWindows;
    this.qualityLimitationReason = decision.qualityLimitationReason;

    if (decision.kind === 'downgrade' || decision.kind === 'upgrade') {
      this.applyProfile(decision.nextProfileIndex, decision.qualityLimitationReason);
    }

    if (decision.shouldResetWindow) {
      this.resetAdaptationWindow();
    }
  }

  private async captureFrame(): Promise<void> {
    if (!this.active || this.capturePending || !this.videoEl || !this.canvas || !this.ctx || !this.encoder) return;
    if (this.videoEl.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) return;
    if (this.encoder.encodeQueueSize > CAMERA_ENCODER_QUEUE_LIMIT) {
      this.telemetry.recordEncoderQueueDrop();
      return;
    }

    const profile = this.supportedProfiles[this.activeProfileIndex];
    if (!profile) return;

    this.capturePending = true;
    try {
      this.ctx.drawImage(this.videoEl, 0, 0, this.canvas.width, this.canvas.height);
      this.telemetry.recordCapture();
      const timestampUs = this.frameTimestampUs;
      this.frameTimestampUs += Math.round(1_000_000 / profile.fps);
      const frame = new VideoFrame(this.canvas, { timestamp: timestampUs });
      const encodedFrames = this.telemetry.getFramesEncoded();
      const keyFrame = this.forceKeyframe
        || (encodedFrames > 0 && (encodedFrames % profile.keyframeInterval) === 0);
      this.forceKeyframe = false;
      this.encodeStarts.set(timestampUs, performance.now());
      try {
        this.encoder.encode(frame, { keyFrame });
      } finally {
        frame.close();
      }
    } catch (e) {
      console.error('[bpane] camera frame capture failed:', e);
    } finally {
      this.capturePending = false;
    }
  }
}
