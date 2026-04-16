import { describe, expect, it } from 'vitest';

import { evaluateCameraAdaptation } from '../camera/camera-adaptation-policy.js';

describe('evaluateCameraAdaptation', () => {
  it('downgrades under bandwidth pressure when a lower rung exists', () => {
    expect(evaluateCameraAdaptation({
      activeProfileIndex: 0,
      profileCount: 3,
      currentProfileFps: 30,
      stableWindows: 0,
      qualityLimitationReason: 'none',
      windowStats: {
        transportQueued: 0,
        transportReplaced: 1,
        encoderQueueDrops: 0,
        encodeTimeMs: 0,
        encodedSamples: 0,
      },
    })).toEqual({
      kind: 'downgrade',
      nextProfileIndex: 1,
      nextStableWindows: 0,
      qualityLimitationReason: 'bandwidth',
      shouldResetWindow: true,
    });
  });

  it('prefers cpu pressure over bandwidth when both are present', () => {
    expect(evaluateCameraAdaptation({
      activeProfileIndex: 0,
      profileCount: 3,
      currentProfileFps: 30,
      stableWindows: 2,
      qualityLimitationReason: 'bandwidth',
      windowStats: {
        transportQueued: 8,
        transportReplaced: 1,
        encoderQueueDrops: 1,
        encodeTimeMs: 0,
        encodedSamples: 0,
      },
    })).toEqual({
      kind: 'downgrade',
      nextProfileIndex: 1,
      nextStableWindows: 0,
      qualityLimitationReason: 'cpu',
      shouldResetWindow: true,
    });
  });

  it('holds the lowest rung and preserves the limitation reason when no downgrade remains', () => {
    expect(evaluateCameraAdaptation({
      activeProfileIndex: 2,
      profileCount: 3,
      currentProfileFps: 18,
      stableWindows: 1,
      qualityLimitationReason: 'none',
      windowStats: {
        transportQueued: 0,
        transportReplaced: 0,
        encoderQueueDrops: 1,
        encodeTimeMs: 0,
        encodedSamples: 0,
      },
    })).toEqual({
      kind: 'hold',
      nextStableWindows: 0,
      qualityLimitationReason: 'cpu',
      shouldResetWindow: true,
    });
  });

  it('clears the limitation reason after two stable windows without upgrading', () => {
    expect(evaluateCameraAdaptation({
      activeProfileIndex: 2,
      profileCount: 3,
      currentProfileFps: 18,
      stableWindows: 1,
      qualityLimitationReason: 'bandwidth',
      windowStats: {
        transportQueued: 0,
        transportReplaced: 0,
        encoderQueueDrops: 0,
        encodeTimeMs: 0,
        encodedSamples: 0,
      },
    })).toEqual({
      kind: 'hold',
      nextStableWindows: 2,
      qualityLimitationReason: 'none',
      shouldResetWindow: true,
    });
  });

  it('upgrades after enough stable windows and low encode time', () => {
    expect(evaluateCameraAdaptation({
      activeProfileIndex: 1,
      profileCount: 3,
      currentProfileFps: 24,
      stableWindows: 2,
      qualityLimitationReason: 'bandwidth',
      windowStats: {
        transportQueued: 0,
        transportReplaced: 0,
        encoderQueueDrops: 0,
        encodeTimeMs: 5,
        encodedSamples: 1,
      },
    })).toEqual({
      kind: 'upgrade',
      nextProfileIndex: 0,
      nextStableWindows: 0,
      qualityLimitationReason: 'none',
      shouldResetWindow: true,
    });
  });
});
