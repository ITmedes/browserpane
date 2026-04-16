export type QualityLimitationReason = 'none' | 'bandwidth' | 'cpu';

export type CameraAdaptationWindowStats = {
  transportQueued: number;
  transportReplaced: number;
  encoderQueueDrops: number;
  encodeTimeMs: number;
  encodedSamples: number;
};

export type CameraAdaptationInput = {
  activeProfileIndex: number;
  profileCount: number;
  currentProfileFps: number;
  stableWindows: number;
  qualityLimitationReason: QualityLimitationReason;
  windowStats: CameraAdaptationWindowStats;
};

export type CameraAdaptationDecision =
  | {
      kind: 'downgrade';
      nextProfileIndex: number;
      nextStableWindows: 0;
      qualityLimitationReason: Exclude<QualityLimitationReason, 'none'>;
      shouldResetWindow: true;
    }
  | {
      kind: 'upgrade';
      nextProfileIndex: number;
      nextStableWindows: 0;
      qualityLimitationReason: 'none';
      shouldResetWindow: true;
    }
  | {
      kind: 'hold';
      nextStableWindows: number;
      qualityLimitationReason: QualityLimitationReason;
      shouldResetWindow: true;
    };

const CAMERA_STABLE_WINDOWS_FOR_UPGRADE = 3;

export function evaluateCameraAdaptation(
  input: CameraAdaptationInput,
): CameraAdaptationDecision {
  const frameBudgetMs = 1000 / input.currentProfileFps;
  const averageEncodeTimeMs = input.windowStats.encodedSamples > 0
    ? input.windowStats.encodeTimeMs / input.windowStats.encodedSamples
    : 0;
  const cpuLimited = input.windowStats.encoderQueueDrops > 0
    || (input.windowStats.encodedSamples > 0 && averageEncodeTimeMs > frameBudgetMs * 0.7);
  const bandwidthLimited = input.windowStats.transportReplaced > 0
    || input.windowStats.transportQueued > Math.max(3, Math.ceil(input.currentProfileFps / 4));

  if (cpuLimited || bandwidthLimited) {
    const qualityLimitationReason: Exclude<QualityLimitationReason, 'none'> = cpuLimited ? 'cpu' : 'bandwidth';
    if (input.activeProfileIndex < input.profileCount - 1) {
      return {
        kind: 'downgrade',
        nextProfileIndex: input.activeProfileIndex + 1,
        nextStableWindows: 0,
        qualityLimitationReason,
        shouldResetWindow: true,
      };
    }

    return {
      kind: 'hold',
      nextStableWindows: 0,
      qualityLimitationReason,
      shouldResetWindow: true,
    };
  }

  const nextStableWindows = input.stableWindows + 1;
  if (
    input.activeProfileIndex > 0
    && nextStableWindows >= CAMERA_STABLE_WINDOWS_FOR_UPGRADE
    && averageEncodeTimeMs < frameBudgetMs * 0.35
  ) {
    return {
      kind: 'upgrade',
      nextProfileIndex: input.activeProfileIndex - 1,
      nextStableWindows: 0,
      qualityLimitationReason: 'none',
      shouldResetWindow: true,
    };
  }

  return {
    kind: 'hold',
    nextStableWindows,
    qualityLimitationReason: nextStableWindows >= 2 ? 'none' : input.qualityLimitationReason,
    shouldResetWindow: true,
  };
}
