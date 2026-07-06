import { describe, expect, it, vi } from 'vitest';

import type {
  BrowserSessionHandle,
  BrowserSessionStatsSnapshot,
  BrowserSessionTileRuntimeStats,
} from './browser-session-types';
import {
  canSampleSessionPreviewMetrics,
  formatBytes,
  formatRate,
  SessionPreviewMetricsSampler,
} from './session-preview-metrics';

describe('SessionPreviewMetricsSampler', () => {
  it('captures browser transition deltas from the BrowserPane runtime handle', () => {
    const metrics = createMutableMetrics();
    const handle = createMetricsHandle(metrics);
    const sampler = new SessionPreviewMetricsSampler(handle);

    sampler.start(1_000);
    metrics.frameCount = 25;
    metrics.tileRuntime = {
      ...metrics.tileRuntime,
      hits: 30,
      misses: 5,
      hitRate: 85.7,
      bytes: 64_000,
      evictions: 2,
      lastBatchCommands: 12,
      maxBatchCommands: 16,
      pendingCommandsHighWaterMark: 7,
    };
    metrics.sessionStats = {
      ...metrics.sessionStats,
      transfer: {
        ...metrics.sessionStats.transfer,
        rxBytes: 120_000,
        txBytes: 12_000,
      },
      tiles: {
        ...metrics.sessionStats.tiles,
        commandBytes: 45_000,
        imageCommands: 12,
        videoCommands: 3,
        drawCommands: 15,
        totalCommands: 30,
        cacheHitRateObserved: 85.7,
        scrollComposition: {
          ...metrics.sessionStats.tiles.scrollComposition,
          scrollBatches: 4,
          scrollPotentialTiles: 100,
          scrollSavedTiles: 75,
        },
        scrollHealth: {
          ...metrics.sessionStats.tiles.scrollHealth,
          hostScrollBatchesTotal: 4,
          hostScrollFallbacksTotal: 1,
          hostScrollResidualFullRepaintsTotal: 1,
          hostFallbackRateRecent20: 25,
          hostFallbackRateRecent50: 25,
        },
      },
      video: {
        ...metrics.sessionStats.video,
        datagrams: 8,
        datagramBytes: 24_000,
        droppedFrames: 1,
      },
    };

    const summary = sampler.update(3_000);

    expect(summary.status).toBe('running');
    expect(summary.durationMs).toBe(2_000);
    expect(summary.frameCount).toBe(25);
    expect(summary.averageFps).toBe(12.5);
    expect(summary.transfer.avgRxRate).toBe(60_000);
    expect(summary.transfer.avgTileRate).toBe(22_500);
    expect(summary.tiles).toMatchObject({
      totalCommands: 30,
      imageCommands: 12,
      videoCommands: 3,
      drawCommands: 15,
      cacheHitRateObserved: 85.7,
      cacheHits: 30,
      cacheMisses: 5,
      cacheEvictions: 2,
      maxBatchCommands: 16,
      maxPendingCommands: 7,
    });
    expect(summary.scroll).toMatchObject({
      scrollBatches: 4,
      savedRate: 75,
      hostFallbackRate: 25,
      dominantHostFallbackReason: 'residual-full-repaint',
    });
    expect(summary.video).toEqual({ datagrams: 8, droppedFrames: 1 });
    expect(summary.render.label).toContain('webgl2 (hardware-accelerated)');
  });

  it('detects whether the handle exposes runtime metrics', () => {
    expect(canSampleSessionPreviewMetrics({ disconnect: vi.fn() })).toBe(false);
    expect(canSampleSessionPreviewMetrics(createMetricsHandle(createMutableMetrics()))).toBe(true);
  });

  it('formats byte rates for compact operator display', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatRate(2048)).toBe('2.0 KB/s');
  });
});

type MutableMetrics = {
  frameCount: number;
  tileRuntime: BrowserSessionTileRuntimeStats;
  sessionStats: BrowserSessionStatsSnapshot;
};

function createMetricsHandle(metrics: MutableMetrics): BrowserSessionHandle {
  return {
    disconnect: vi.fn(),
    getFrameCount: () => metrics.frameCount,
    getTileCacheStats: () => metrics.tileRuntime,
    getSessionStats: () => metrics.sessionStats,
    getRenderDiagnostics: () => ({
      backend: 'webgl2',
      reason: 'hardware-accelerated',
      renderer: 'ANGLE (Apple, Apple M3, Unspecified Version)',
      vendor: 'Apple',
      software: false,
    }),
  };
}

function createMutableMetrics(): MutableMetrics {
  return {
    frameCount: 0,
    tileRuntime: {
      hits: 0,
      misses: 0,
      hitRate: 0,
      size: 0,
      bytes: 0,
      evictions: 0,
      fills: 0,
      qoiDecodes: 0,
      qoiRedundant: 0,
      qoiRedundantBytes: 0,
      zstdDecodes: 0,
      zstdRedundant: 0,
      zstdRedundantBytes: 0,
      cacheMisses: 0,
      scrollCopies: 0,
      batchesQueued: 0,
      totalBatchCommands: 0,
      maxBatchCommands: 0,
      lastBatchCommands: 0,
      currentPendingCommands: 0,
      pendingCommandsHighWaterMark: 0,
    },
    sessionStats: {
      elapsedMs: 0,
      transfer: {
        rxBytes: 0,
        txBytes: 0,
        rxFrames: 0,
        txFrames: 0,
        rxByChannel: {},
        txByChannel: {},
      },
      tiles: {
        commandBytes: 0,
        imageCommands: 0,
        videoCommands: 0,
        drawCommands: 0,
        totalCommands: 0,
        cacheHitsObserved: 0,
        cacheMissesObserved: 0,
        cacheHitRateObserved: 0,
        cacheSizeObserved: 0,
        redundantQoiCommands: 0,
        redundantQoiBytes: 0,
        scrollComposition: {
          scrollBatches: 0,
          subTileScrollBatches: 0,
          scrollPotentialTiles: 0,
          scrollSavedTiles: 0,
          subTileScrollPotentialTiles: 0,
          subTileScrollSavedTiles: 0,
          subTileScrollReuseRate: 0,
          subTileComposeCandidateRate: 0,
        },
        scrollHealth: {
          hostScrollBatchesTotal: 0,
          hostScrollFallbacksTotal: 0,
          hostScrollNonQuantizedFallbacksTotal: 0,
          hostScrollResidualFullRepaintsTotal: 0,
          hostScrollResidualInteriorLimitFallbacksTotal: 0,
          hostScrollResidualLowSavedRatioFallbacksTotal: 0,
          hostScrollResidualLargeRowShiftFallbacksTotal: 0,
          hostScrollResidualOtherFallbacksTotal: 0,
          hostScrollZeroSavedBatchesTotal: 0,
          hostScrollSplitRegionBatchesTotal: 0,
          hostScrollStickyBandBatchesTotal: 0,
          hostScrollChromeTilesTotal: 0,
          hostScrollExposedStripTilesTotal: 0,
          hostScrollInteriorResidualTilesTotal: 0,
          hostScrollEdgeStripResidualTilesTotal: 0,
          hostScrollSmallEdgeStripResidualTilesTotal: 0,
          hostScrollSmallEdgeStripResidualRowsTotal: 0,
          hostScrollSmallEdgeStripResidualAreaPxTotal: 0,
          hostSentHashEntries: 0,
          hostSentHashEvictionsTotal: 0,
          hostCacheMissReportsTotal: 0,
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
      },
      video: {
        decodedFrames: 0,
        droppedFrames: 0,
        datagrams: 0,
        datagramBytes: 0,
      },
    },
  };
}
