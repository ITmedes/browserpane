type CameraSendResult = 'sent' | 'queued' | 'replaced';
type SendCameraFrameFn = (payload: Uint8Array) => CameraSendResult;
type QualityLimitationReason = 'none' | 'bandwidth' | 'cpu';

export interface CameraTelemetryProfile {
  name: string;
  width: number;
  height: number;
  fps: number;
  bitrate: number;
  smooth: boolean | null;
  powerEfficient: boolean | null;
}

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

interface CameraProfile extends CameraTelemetryProfile {
  codec: string;
  keyframeInterval: number;
}

const CAMERA_WEBRTC_CONTENT_TYPE = 'video/H264;level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f';
const CAMERA_CAPTURE_WIDTH = 1280;
const CAMERA_CAPTURE_HEIGHT = 720;
const CAMERA_CAPTURE_FRAMERATE = 30;
const CAMERA_ADAPT_INTERVAL_MS = 2000;
const CAMERA_STABLE_WINDOWS_FOR_UPGRADE = 3;
const CAMERA_ENCODER_QUEUE_LIMIT = 2;

const CAMERA_PROFILES: CameraProfile[] = [
  {
    name: 'hd720p',
    width: 1280,
    height: 720,
    fps: 30,
    bitrate: 1_600_000,
    keyframeInterval: 30,
    codec: 'avc1.42001f',
    smooth: null,
    powerEfficient: null,
  },
  {
    name: 'qhd540p',
    width: 960,
    height: 540,
    fps: 24,
    bitrate: 950_000,
    keyframeInterval: 24,
    codec: 'avc1.42001f',
    smooth: null,
    powerEfficient: null,
  },
  {
    name: 'nhd360p',
    width: 640,
    height: 360,
    fps: 18,
    bitrate: 450_000,
    keyframeInterval: 18,
    codec: 'avc1.42001e',
    smooth: null,
    powerEfficient: null,
  },
];

export class CameraController {
  private static supportCache: Promise<CameraProfile[]> | null = null;

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

  private framesCaptured = 0;
  private framesEncoded = 0;
  private keyframesEncoded = 0;
  private encodedBytes = 0;
  private transportFramesQueued = 0;
  private transportFramesReplaced = 0;
  private encoderQueueDrops = 0;
  private totalEncodeTimeMs = 0;
  private maxEncodeTimeMs = 0;
  private profileUpgrades = 0;
  private profileDowngrades = 0;
  private reconfigurations = 0;

  private windowTransportQueued = 0;
  private windowTransportReplaced = 0;
  private windowEncoderQueueDrops = 0;
  private windowEncodeTimeMs = 0;
  private windowEncodedSamples = 0;

  constructor(sendFrame: SendCameraFrameFn) {
    this.sendFrame = sendFrame;
  }

  static async isSupported(): Promise<boolean> {
    const supported = await CameraController.getSupportedProfiles();
    return supported.length > 0;
  }

  static async getSupportedProfiles(): Promise<CameraProfile[]> {
    if (!CameraController.supportCache) {
      CameraController.supportCache = CameraController.probeSupportedProfiles();
    }
    return CameraController.supportCache;
  }

  async startCamera(): Promise<void> {
    if (this.active) return;
    try {
      this.supportedProfiles = await CameraController.getSupportedProfiles();
      const initialProfile = this.supportedProfiles[0];
      if (!initialProfile) {
        throw new Error('camera video encoding is not supported in this browser');
      }

      this.stream = await navigator.mediaDevices.getUserMedia({
        video: {
          width: { ideal: CAMERA_CAPTURE_WIDTH },
          height: { ideal: CAMERA_CAPTURE_HEIGHT },
          frameRate: { ideal: CAMERA_CAPTURE_FRAMERATE, max: CAMERA_CAPTURE_FRAMERATE + 5 },
        },
        audio: false,
      });

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
    const averageEncodeTimeMs = this.framesEncoded > 0 ? this.totalEncodeTimeMs / this.framesEncoded : 0;
    return {
      supported: this.supportedProfiles.length > 0,
      active: this.active,
      profile,
      qualityLimitationReason: this.qualityLimitationReason,
      framesCaptured: this.framesCaptured,
      framesEncoded: this.framesEncoded,
      keyframesEncoded: this.keyframesEncoded,
      encodedBytes: this.encodedBytes,
      transportFramesQueued: this.transportFramesQueued,
      transportFramesReplaced: this.transportFramesReplaced,
      encoderQueueDrops: this.encoderQueueDrops,
      averageEncodeTimeMs,
      maxEncodeTimeMs: this.maxEncodeTimeMs,
      profileUpgrades: this.profileUpgrades,
      profileDowngrades: this.profileDowngrades,
      reconfigurations: this.reconfigurations,
    };
  }

  private static async probeSupportedProfiles(): Promise<CameraProfile[]> {
    if (
      typeof navigator === 'undefined'
      || !navigator.mediaDevices?.getUserMedia
      || typeof VideoEncoder === 'undefined'
      || typeof VideoFrame === 'undefined'
    ) {
      return [];
    }

    const supported: CameraProfile[] = [];
    for (const profile of CAMERA_PROFILES) {
      const config = cameraEncoderConfig(profile);
      try {
        const encoderSupport = await VideoEncoder.isConfigSupported(config);
        if (!encoderSupport.supported) {
          continue;
        }
        const runtimeProfile = { ...profile };
        const mediaCapabilities = (navigator as Navigator & {
          mediaCapabilities?: {
            encodingInfo?: (configuration: unknown) => Promise<{
              supported: boolean;
              smooth?: boolean;
              powerEfficient?: boolean;
            }>;
          };
        }).mediaCapabilities;
        if (mediaCapabilities?.encodingInfo) {
          try {
            const info = await mediaCapabilities.encodingInfo({
              type: 'webrtc',
              video: {
                contentType: CAMERA_WEBRTC_CONTENT_TYPE,
                width: profile.width,
                height: profile.height,
                bitrate: profile.bitrate,
                framerate: profile.fps,
              },
            });
            if (!info.supported) {
              continue;
            }
            runtimeProfile.smooth = typeof info.smooth === 'boolean' ? info.smooth : null;
            runtimeProfile.powerEfficient = typeof info.powerEfficient === 'boolean' ? info.powerEfficient : null;
          } catch {
            // Ignore media-capabilities probe failures and rely on VideoEncoder support.
          }
        }
        supported.push(runtimeProfile);
      } catch {
        // Ignore this rung and try the next one down.
      }
    }

    const smoothFirst = supported.filter((profile) => profile.smooth !== false);
    return (smoothFirst.length > 0 ? smoothFirst : supported)
      .concat(supported.filter((profile) => profile.smooth === false && !smoothFirst.includes(profile)));
  }

  private resetTelemetry(): void {
    this.qualityLimitationReason = 'none';
    this.stableWindows = 0;
    this.frameTimestampUs = 0;
    this.forceKeyframe = true;
    this.framesCaptured = 0;
    this.framesEncoded = 0;
    this.keyframesEncoded = 0;
    this.encodedBytes = 0;
    this.transportFramesQueued = 0;
    this.transportFramesReplaced = 0;
    this.encoderQueueDrops = 0;
    this.totalEncodeTimeMs = 0;
    this.maxEncodeTimeMs = 0;
    this.profileUpgrades = 0;
    this.profileDowngrades = 0;
    this.reconfigurations = 0;
    this.resetAdaptationWindow();
  }

  private resetAdaptationWindow(): void {
    this.windowTransportQueued = 0;
    this.windowTransportReplaced = 0;
    this.windowEncoderQueueDrops = 0;
    this.windowEncodeTimeMs = 0;
    this.windowEncodedSamples = 0;
  }

  private applyProfile(index: number, reason: QualityLimitationReason): void {
    const profile = this.supportedProfiles[index];
    if (!profile) return;

    const previousIndex = this.activeProfileIndex;
    if (previousIndex >= 0 && previousIndex !== index) {
      this.reconfigurations += 1;
      if (index > previousIndex) {
        this.profileDowngrades += 1;
      } else {
        this.profileUpgrades += 1;
      }
    }

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
    this.encoder.configure(cameraEncoderConfig(profile));
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
    const startedAt = this.encodeStarts.get(chunk.timestamp);
    if (typeof startedAt === 'number') {
      this.encodeStarts.delete(chunk.timestamp);
      const encodeTimeMs = performance.now() - startedAt;
      this.totalEncodeTimeMs += encodeTimeMs;
      this.windowEncodeTimeMs += encodeTimeMs;
      this.windowEncodedSamples += 1;
      this.maxEncodeTimeMs = Math.max(this.maxEncodeTimeMs, encodeTimeMs);
    }

    const payload = new Uint8Array(chunk.byteLength);
    chunk.copyTo(payload);
    this.framesEncoded += 1;
    if (chunk.type === 'key') {
      this.keyframesEncoded += 1;
    }
    this.encodedBytes += payload.byteLength;

    const result = this.sendFrame(payload);
    if (result === 'queued') {
      this.transportFramesQueued += 1;
      this.windowTransportQueued += 1;
    } else if (result === 'replaced') {
      this.transportFramesReplaced += 1;
      this.windowTransportReplaced += 1;
    }
  }

  private evaluateAdaptation(): void {
    const profile = this.supportedProfiles[this.activeProfileIndex];
    if (!this.active || !profile) return;

    const frameBudgetMs = 1000 / profile.fps;
    const averageEncodeTimeMs = this.windowEncodedSamples > 0
      ? this.windowEncodeTimeMs / this.windowEncodedSamples
      : 0;
    const cpuLimited = this.windowEncoderQueueDrops > 0
      || (this.windowEncodedSamples > 0 && averageEncodeTimeMs > frameBudgetMs * 0.7);
    const bandwidthLimited = this.windowTransportReplaced > 0
      || this.windowTransportQueued > Math.max(3, Math.ceil(profile.fps / 4));

    if (cpuLimited || bandwidthLimited) {
      this.stableWindows = 0;
      const reason: QualityLimitationReason = cpuLimited ? 'cpu' : 'bandwidth';
      if (this.activeProfileIndex < this.supportedProfiles.length - 1) {
        this.applyProfile(this.activeProfileIndex + 1, reason);
      } else {
        this.qualityLimitationReason = reason;
      }
      this.resetAdaptationWindow();
      return;
    }

    this.stableWindows += 1;
    if (
      this.activeProfileIndex > 0
      && this.stableWindows >= CAMERA_STABLE_WINDOWS_FOR_UPGRADE
      && averageEncodeTimeMs < frameBudgetMs * 0.35
    ) {
      this.applyProfile(this.activeProfileIndex - 1, 'none');
      this.stableWindows = 0;
      this.resetAdaptationWindow();
      return;
    }

    if (this.stableWindows >= 2) {
      this.qualityLimitationReason = 'none';
    }
    this.resetAdaptationWindow();
  }

  private async captureFrame(): Promise<void> {
    if (!this.active || this.capturePending || !this.videoEl || !this.canvas || !this.ctx || !this.encoder) return;
    if (this.videoEl.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) return;
    if (this.encoder.encodeQueueSize > CAMERA_ENCODER_QUEUE_LIMIT) {
      this.encoderQueueDrops += 1;
      this.windowEncoderQueueDrops += 1;
      return;
    }

    const profile = this.supportedProfiles[this.activeProfileIndex];
    if (!profile) return;

    this.capturePending = true;
    try {
      this.ctx.drawImage(this.videoEl, 0, 0, this.canvas.width, this.canvas.height);
      this.framesCaptured += 1;
      const timestampUs = this.frameTimestampUs;
      this.frameTimestampUs += Math.round(1_000_000 / profile.fps);
      const frame = new VideoFrame(this.canvas, { timestamp: timestampUs });
      const keyFrame = this.forceKeyframe
        || (this.framesEncoded > 0 && (this.framesEncoded % profile.keyframeInterval) === 0);
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

function cameraEncoderConfig(profile: CameraProfile): VideoEncoderConfig {
  return {
    codec: profile.codec,
    width: profile.width,
    height: profile.height,
    displayWidth: profile.width,
    displayHeight: profile.height,
    bitrate: profile.bitrate,
    framerate: profile.fps,
    latencyMode: 'realtime',
    avc: {
      format: 'annexb',
    },
  };
}
