import { describe, expect, it } from 'vitest';

import { buildCameraTelemetrySnapshot } from '../camera/camera-telemetry-snapshot.js';

describe('buildCameraTelemetrySnapshot', () => {
  it('builds an active snapshot with copied profile data and accumulated metrics', () => {
    const profile = {
      name: 'hd720p',
      width: 1280,
      height: 720,
      fps: 30,
      bitrate: 1_600_000,
      smooth: true,
      powerEfficient: false,
    };

    const snapshot = buildCameraTelemetrySnapshot({
      supportedProfilesCount: 3,
      active: true,
      profile,
      qualityLimitationReason: 'bandwidth',
      metrics: {
        framesCaptured: 11,
        framesEncoded: 10,
        keyframesEncoded: 2,
        encodedBytes: 2048,
        transportFramesQueued: 4,
        transportFramesReplaced: 1,
        encoderQueueDrops: 3,
        averageEncodeTimeMs: 6.5,
        maxEncodeTimeMs: 12,
        profileUpgrades: 1,
        profileDowngrades: 2,
        reconfigurations: 3,
      },
    });

    expect(snapshot).toEqual({
      supported: true,
      active: true,
      profile: {
        name: 'hd720p',
        width: 1280,
        height: 720,
        fps: 30,
        bitrate: 1_600_000,
        smooth: true,
        powerEfficient: false,
      },
      qualityLimitationReason: 'bandwidth',
      framesCaptured: 11,
      framesEncoded: 10,
      keyframesEncoded: 2,
      encodedBytes: 2048,
      transportFramesQueued: 4,
      transportFramesReplaced: 1,
      encoderQueueDrops: 3,
      averageEncodeTimeMs: 6.5,
      maxEncodeTimeMs: 12,
      profileUpgrades: 1,
      profileDowngrades: 2,
      reconfigurations: 3,
    });
    expect(snapshot.profile).not.toBe(profile);
  });

  it('builds an inactive unsupported snapshot without a profile', () => {
    expect(buildCameraTelemetrySnapshot({
      supportedProfilesCount: 0,
      active: false,
      profile: null,
      qualityLimitationReason: 'none',
      metrics: {
        framesCaptured: 0,
        framesEncoded: 0,
        keyframesEncoded: 0,
        encodedBytes: 0,
        transportFramesQueued: 0,
        transportFramesReplaced: 0,
        encoderQueueDrops: 0,
        averageEncodeTimeMs: 0,
        maxEncodeTimeMs: 0,
        profileUpgrades: 0,
        profileDowngrades: 0,
        reconfigurations: 0,
      },
    })).toEqual({
      supported: false,
      active: false,
      profile: null,
      qualityLimitationReason: 'none',
      framesCaptured: 0,
      framesEncoded: 0,
      keyframesEncoded: 0,
      encodedBytes: 0,
      transportFramesQueued: 0,
      transportFramesReplaced: 0,
      encoderQueueDrops: 0,
      averageEncodeTimeMs: 0,
      maxEncodeTimeMs: 0,
      profileUpgrades: 0,
      profileDowngrades: 0,
      reconfigurations: 0,
    });
  });
});
