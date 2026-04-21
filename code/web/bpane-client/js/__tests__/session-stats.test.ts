import { describe, expect, it } from 'vitest';
import { SessionStats } from '../session-stats.js';
import { CH_VIDEO_IN } from '../protocol.js';

const tileCacheRuntime = {
  hits: 0,
  misses: 0,
  hitRate: 0,
  size: 0,
  qoiRedundant: 0,
  qoiRedundantBytes: 0,
  zstdRedundant: 0,
  zstdRedundantBytes: 0,
};

describe('SessionStats host scroll health', () => {
  it('reports rolling host fallback windows from cumulative totals', () => {
    const stats = new SessionStats();

    stats.recordHostScrollStats(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    stats.recordHostScrollStats(10, 2, 100, 80, 1, 1, 1, 0, 0, 0, 2, 3, 1, 12, 4, 8);
    stats.recordHostScrollStats(30, 8, 300, 210, 3, 5, 2, 1, 1, 1, 4, 8, 3, 30, 10, 18);

    const snapshot = stats.getSessionStats(tileCacheRuntime);
    const health = snapshot.tiles.scrollHealth;

    expect(health.hostFallbackRate).toBeCloseTo((8 / 30) * 100, 5);
    expect(health.hostFallbackRateRecent20Batches).toBe(20);
    expect(health.hostFallbackRateRecent20).toBeCloseTo(30, 5);
    expect(health.hostFallbackRateRecent50Batches).toBe(30);
    expect(health.hostFallbackRateRecent50).toBeCloseTo((8 / 30) * 100, 5);
    expect(health.hostScrollNonQuantizedFallbacksTotal).toBe(3);
    expect(health.hostScrollResidualFullRepaintsTotal).toBe(5);
    expect(health.hostScrollResidualInteriorLimitFallbacksTotal).toBe(2);
    expect(health.hostScrollResidualLowSavedRatioFallbacksTotal).toBe(1);
    expect(health.hostScrollResidualLargeRowShiftFallbacksTotal).toBe(1);
    expect(health.hostScrollResidualOtherFallbacksTotal).toBe(1);
    expect(health.hostScrollZeroSavedBatchesTotal).toBe(4);
    expect(health.hostScrollEdgeStripResidualTilesTotal).toBe(0);
    expect(health.hostScrollSmallEdgeStripResidualTilesTotal).toBe(0);
    expect(health.hostScrollSmallEdgeStripResidualRowsTotal).toBe(0);
    expect(health.hostScrollSmallEdgeStripResidualAreaPxTotal).toBe(0);
  });

  it('clears rolling history when host counters reset', () => {
    const stats = new SessionStats();

    stats.recordHostScrollStats(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    stats.recordHostScrollStats(12, 3, 120, 90, 1, 2, 1, 1, 0, 0, 3, 4, 2, 18, 6, 11);
    stats.recordHostScrollStats(1, 0, 10, 10, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0);

    const snapshot = stats.getSessionStats(tileCacheRuntime);
    const health = snapshot.tiles.scrollHealth;

    expect(health.hostScrollBatchesTotal).toBe(1);
    expect(health.hostScrollFallbacksTotal).toBe(0);
    expect(health.hostScrollNonQuantizedFallbacksTotal).toBe(0);
    expect(health.hostScrollResidualFullRepaintsTotal).toBe(0);
    expect(health.hostScrollResidualInteriorLimitFallbacksTotal).toBe(0);
    expect(health.hostScrollResidualLowSavedRatioFallbacksTotal).toBe(0);
    expect(health.hostScrollResidualLargeRowShiftFallbacksTotal).toBe(0);
    expect(health.hostScrollResidualOtherFallbacksTotal).toBe(0);
    expect(health.hostScrollZeroSavedBatchesTotal).toBe(1);
    expect(health.hostScrollEdgeStripResidualTilesTotal).toBe(0);
    expect(health.hostScrollSmallEdgeStripResidualTilesTotal).toBe(0);
    expect(health.hostScrollSmallEdgeStripResidualRowsTotal).toBe(0);
    expect(health.hostScrollSmallEdgeStripResidualAreaPxTotal).toBe(0);
    expect(health.hostFallbackRateRecent20Batches).toBe(0);
    expect(health.hostFallbackRateRecent20).toBe(0);
    expect(health.hostFallbackRateRecent50Batches).toBe(0);
    expect(health.hostFallbackRateRecent50).toBe(0);
  });

  it('includes camera telemetry alongside transport counters', () => {
    const stats = new SessionStats();
    stats.recordTx(CH_VIDEO_IN, 2048);
    stats.recordTx(CH_VIDEO_IN, 1024);

    const snapshot = stats.getSessionStats(tileCacheRuntime, {
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
      framesCaptured: 42,
      framesEncoded: 40,
      keyframesEncoded: 2,
      encodedBytes: 3000,
      transportFramesQueued: 6,
      transportFramesReplaced: 4,
      encoderQueueDrops: 1,
      averageEncodeTimeMs: 5.5,
      maxEncodeTimeMs: 11,
      profileUpgrades: 0,
      profileDowngrades: 1,
      reconfigurations: 1,
    });

    expect(snapshot.transfer.txByChannel.videoIn).toEqual({
      bytes: 3072,
      frames: 2,
    });
    expect(snapshot.camera).toMatchObject({
      supported: true,
      active: true,
      qualityLimitationReason: 'bandwidth',
      transportBytesSent: 3072,
      transportFramesSent: 2,
      transportFramesQueued: 6,
      transportFramesReplaced: 4,
      profileDowngrades: 1,
    });
  });
});
