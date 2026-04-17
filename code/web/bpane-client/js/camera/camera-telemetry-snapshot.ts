import type { QualityLimitationReason } from './camera-adaptation-policy.js';
import type { CameraProfile } from './camera-profile-catalog.js';
import type { CameraTelemetryMetrics } from './camera-telemetry-tracker.js';

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

export type CameraTelemetrySnapshotBuildInput = {
  supportedProfilesCount: number;
  active: boolean;
  profile: CameraTelemetryProfile | null;
  qualityLimitationReason: QualityLimitationReason;
  metrics: CameraTelemetryMetrics;
};

export function buildCameraTelemetrySnapshot(
  input: CameraTelemetrySnapshotBuildInput,
): CameraTelemetrySnapshot {
  return {
    supported: input.supportedProfilesCount > 0,
    active: input.active,
    profile: input.profile ? { ...input.profile } : null,
    qualityLimitationReason: input.qualityLimitationReason,
    ...input.metrics,
  };
}
