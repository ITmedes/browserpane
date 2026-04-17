import {
  evaluateCameraAdaptation,
  type QualityLimitationReason,
} from './camera/camera-adaptation-policy.js';
import {
  CameraCaptureRuntime,
  type SendCameraFrameFn,
} from './camera/camera-capture-runtime.js';
import {
  CameraProfileCatalog,
  type CameraProfile,
} from './camera/camera-profile-catalog.js';
import { CameraSessionResources } from './camera/camera-session-resources.js';
import {
  buildCameraTelemetrySnapshot,
  type CameraTelemetrySnapshot,
} from './camera/camera-telemetry-snapshot.js';
import { CameraTelemetryTracker } from './camera/camera-telemetry-tracker.js';
export type {
  CameraTelemetryProfile,
  CameraTelemetrySnapshot,
} from './camera/camera-telemetry-snapshot.js';

const CAMERA_ADAPT_INTERVAL_MS = 2000;

export class CameraController {
  private readonly sendFrame: SendCameraFrameFn;
  private sessionResources: CameraSessionResources | null = null;
  private captureRuntime: CameraCaptureRuntime | null = null;
  private adaptationTimer: ReturnType<typeof setInterval> | null = null;
  private active = false;
  private activeProfileIndex = -1;
  private supportedProfiles: CameraProfile[] = [];
  private qualityLimitationReason: QualityLimitationReason = 'none';
  private stableWindows = 0;
  private readonly telemetry = new CameraTelemetryTracker();

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

      this.sessionResources = new CameraSessionResources();
      const sessionSnapshot = await this.sessionResources.open();

      this.captureRuntime = new CameraCaptureRuntime({
        videoElement: sessionSnapshot.videoElement,
        canvasElement: sessionSnapshot.canvasElement,
        canvasContext: sessionSnapshot.canvasContext,
        sendFrame: this.sendFrame,
        telemetry: this.telemetry,
        onEncoderError: (error) => {
          console.error('[bpane] camera encoder error:', error);
          this.stopCamera();
        },
      });

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
    if (!this.active && !this.sessionResources?.getSnapshot()) return;
    this.active = false;
    if (this.adaptationTimer) {
      clearInterval(this.adaptationTimer);
      this.adaptationTimer = null;
    }
    this.captureRuntime?.stop();
    this.captureRuntime = null;
    this.sessionResources?.close();
    this.sessionResources = null;
    this.activeProfileIndex = -1;
    this.sendFrame(new Uint8Array());
  }

  destroy(): void {
    this.stopCamera();
  }

  getStats(): CameraTelemetrySnapshot {
    const profile = this.activeProfileIndex >= 0 ? { ...this.supportedProfiles[this.activeProfileIndex] } : null;
    return buildCameraTelemetrySnapshot({
      supportedProfilesCount: this.supportedProfiles.length,
      active: this.active,
      profile,
      qualityLimitationReason: this.qualityLimitationReason,
      metrics: this.telemetry.getMetrics(),
    });
  }

  private resetTelemetry(): void {
    this.qualityLimitationReason = 'none';
    this.stableWindows = 0;
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
    this.captureRuntime?.applyProfile(profile);
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
}
