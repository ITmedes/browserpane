import { describe, expect, it } from 'vitest';
import { CH_VIDEO, CH_VIDEO_IN } from '../protocol.js';
import type {
  TileCacheRuntimeStats,
  TileCommandStats,
} from '../session-stats/models.js';
import { SessionStatsSnapshotBuilder } from '../session-stats/session-stats-snapshot-builder.js';

function createTileCommandStats(): TileCommandStats {
  return {
    gridConfig: 1,
    batchEnd: 1,
    fill: 2,
    qoi: 3,
    zstd: 4,
    cacheHit: 5,
    videoRegion: 6,
    scrollCopy: 7,
    gridOffset: 8,
    scrollStats: 9,
    unknown: 10,
  };
}

function createTileCacheRuntime(): TileCacheRuntimeStats {
  return {
    hits: 11,
    misses: 12,
    hitRate: 47.8,
    size: 13,
    qoiRedundant: 14,
    qoiRedundantBytes: 15,
    zstdRedundant: 16,
    zstdRedundantBytes: 17,
  };
}

describe('SessionStatsSnapshotBuilder', () => {
  it('fills default camera telemetry when camera stats are omitted', () => {
    const snapshot = SessionStatsSnapshotBuilder.build({
      nowMs: 3_000,
      sessionStartAtMs: 2_000,
      frameCount: 4,
      rxBytes: 10,
      txBytes: 11,
      rxFrames: 12,
      txFrames: 13,
      rxChannelBytes: {},
      rxChannelFrames: {},
      txChannelBytes: {},
      txChannelFrames: {},
      videoDatagramsRx: 14,
      videoDatagramBytesRx: 15,
      videoFramesDropped: 16,
      tileCommandBytes: 17,
      tileCommandCounts: createTileCommandStats(),
      scrollBatchStats: {
        scrollBatches: 0,
        subTileScrollBatches: 0,
        scrollUpdateCommands: 0,
        scrollQoiCommands: 0,
        scrollCacheHitCommands: 0,
        scrollFillCommands: 0,
        scrollQoiBytes: 0,
        scrollPotentialTiles: 0,
        scrollSavedTiles: 0,
        subTileScrollUpdateCommands: 0,
        subTileScrollQoiCommands: 0,
        subTileScrollQoiBytes: 0,
        subTileScrollPotentialTiles: 0,
        subTileScrollSavedTiles: 0,
      },
      hostScrollHealth: {
        hostScrollBatchesTotal: 0,
        hostScrollFallbacksTotal: 0,
        hostScrollNonQuantizedFallbacksTotal: 0,
        hostScrollResidualFullRepaintsTotal: 0,
        hostScrollZeroSavedBatchesTotal: 0,
        hostFallbackRate: 0,
        hostFallbackRateRecent20: 0,
        hostFallbackRateRecent50: 0,
        hostFallbackRateRecent20Batches: 0,
        hostFallbackRateRecent50Batches: 0,
        hostScrollPotentialTilesTotal: 0,
        hostScrollSavedTilesTotal: 0,
        hostScrollSavedRate: 0,
        lastHostScrollStatsAtMs: 0,
      },
      tileCacheRuntime: createTileCacheRuntime(),
    });

    expect(snapshot.elapsedMs).toBe(1_000);
    expect(snapshot.camera).toEqual({
      supported: false,
      active: false,
      profile: null,
      qualityLimitationReason: 'none',
      transportBytesSent: 0,
      transportFramesSent: 0,
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

  it('includes known and unknown channel labels in transfer snapshots', () => {
    const snapshot = SessionStatsSnapshotBuilder.build({
      nowMs: 100,
      sessionStartAtMs: 50,
      frameCount: 0,
      rxBytes: 25,
      txBytes: 26,
      rxFrames: 27,
      txFrames: 28,
      rxChannelBytes: { [CH_VIDEO]: 7, 99: 9 },
      rxChannelFrames: { [CH_VIDEO]: 1, 99: 2 },
      txChannelBytes: { [CH_VIDEO_IN]: 11 },
      txChannelFrames: { [CH_VIDEO_IN]: 4 },
      videoDatagramsRx: 0,
      videoDatagramBytesRx: 0,
      videoFramesDropped: 0,
      tileCommandBytes: 0,
      tileCommandCounts: createTileCommandStats(),
      scrollBatchStats: {
        scrollBatches: 0,
        subTileScrollBatches: 0,
        scrollUpdateCommands: 0,
        scrollQoiCommands: 0,
        scrollCacheHitCommands: 0,
        scrollFillCommands: 0,
        scrollQoiBytes: 0,
        scrollPotentialTiles: 0,
        scrollSavedTiles: 0,
        subTileScrollUpdateCommands: 0,
        subTileScrollQoiCommands: 0,
        subTileScrollQoiBytes: 0,
        subTileScrollPotentialTiles: 0,
        subTileScrollSavedTiles: 0,
      },
      hostScrollHealth: {
        hostScrollBatchesTotal: 0,
        hostScrollFallbacksTotal: 0,
        hostScrollNonQuantizedFallbacksTotal: 0,
        hostScrollResidualFullRepaintsTotal: 0,
        hostScrollZeroSavedBatchesTotal: 0,
        hostFallbackRate: 0,
        hostFallbackRateRecent20: 0,
        hostFallbackRateRecent50: 0,
        hostFallbackRateRecent20Batches: 0,
        hostFallbackRateRecent50Batches: 0,
        hostScrollPotentialTilesTotal: 0,
        hostScrollSavedTilesTotal: 0,
        hostScrollSavedRate: 0,
        lastHostScrollStatsAtMs: 0,
      },
      tileCacheRuntime: createTileCacheRuntime(),
    });

    expect(snapshot.transfer.rxByChannel.video).toEqual({ bytes: 7, frames: 1 });
    expect(snapshot.transfer.rxByChannel.ch99).toEqual({ bytes: 9, frames: 2 });
    expect(snapshot.transfer.txByChannel.videoIn).toEqual({ bytes: 11, frames: 4 });
  });

  it('returns zero composition rates when the relevant denominators are zero', () => {
    const snapshot = SessionStatsSnapshotBuilder.build({
      nowMs: 100,
      sessionStartAtMs: 100,
      frameCount: 0,
      rxBytes: 0,
      txBytes: 0,
      rxFrames: 0,
      txFrames: 0,
      rxChannelBytes: {},
      rxChannelFrames: {},
      txChannelBytes: {},
      txChannelFrames: {},
      videoDatagramsRx: 0,
      videoDatagramBytesRx: 0,
      videoFramesDropped: 0,
      tileCommandBytes: 0,
      tileCommandCounts: createTileCommandStats(),
      scrollBatchStats: {
        scrollBatches: 0,
        subTileScrollBatches: 0,
        scrollUpdateCommands: 0,
        scrollQoiCommands: 0,
        scrollCacheHitCommands: 0,
        scrollFillCommands: 0,
        scrollQoiBytes: 0,
        scrollPotentialTiles: 0,
        scrollSavedTiles: 0,
        subTileScrollUpdateCommands: 0,
        subTileScrollQoiCommands: 0,
        subTileScrollQoiBytes: 0,
        subTileScrollPotentialTiles: 0,
        subTileScrollSavedTiles: 0,
      },
      hostScrollHealth: {
        hostScrollBatchesTotal: 0,
        hostScrollFallbacksTotal: 0,
        hostScrollNonQuantizedFallbacksTotal: 0,
        hostScrollResidualFullRepaintsTotal: 0,
        hostScrollZeroSavedBatchesTotal: 0,
        hostFallbackRate: 0,
        hostFallbackRateRecent20: 0,
        hostFallbackRateRecent50: 0,
        hostFallbackRateRecent20Batches: 0,
        hostFallbackRateRecent50Batches: 0,
        hostScrollPotentialTilesTotal: 0,
        hostScrollSavedTilesTotal: 0,
        hostScrollSavedRate: 0,
        lastHostScrollStatsAtMs: 0,
      },
      tileCacheRuntime: createTileCacheRuntime(),
    });

    expect(snapshot.tiles.scrollComposition.scrollReuseRate).toBe(0);
    expect(snapshot.tiles.scrollComposition.subTileScrollReuseRate).toBe(0);
    expect(snapshot.tiles.scrollComposition.subTileComposeCandidateRate).toBe(0);
  });
});
