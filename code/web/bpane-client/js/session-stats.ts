/**
 * SessionStats — extracted from BpaneSession.
 *
 * Pure data tracking with no side effects. Holds all transfer counters,
 * tile command counters, scroll batch stats, and host scroll health data.
 */

import { HostScrollHealthTracker } from './session-stats/host-scroll-health-tracker.js';
import type {
  PendingTileBatchStats,
  ScrollBatchStats,
  TileCommandStats,
  TileCacheRuntimeStats,
  SessionStatsSnapshot,
} from './session-stats/models.js';
import { SessionStatsSnapshotBuilder } from './session-stats/session-stats-snapshot-builder.js';
import type { CameraTelemetrySnapshot } from './camera-controller.js';

export type {
  ChannelTransferStats,
  TileCommandStats,
  SessionStatsSnapshot,
  TileCacheRuntimeStats,
} from './session-stats/models.js';

export class SessionStats {
  frameCount = 0;
  sessionStartAtMs = performance.now();
  rxBytes = 0;
  txBytes = 0;
  rxFrames = 0;
  txFrames = 0;
  rxChannelBytes: Record<number, number> = {};
  rxChannelFrames: Record<number, number> = {};
  txChannelBytes: Record<number, number> = {};
  txChannelFrames: Record<number, number> = {};
  videoDatagramsRx = 0;
  videoDatagramBytesRx = 0;
  videoFramesDropped = 0;
  tileCommandBytes = 0;
  tileCommandCounts: TileCommandStats = {
    gridConfig: 0,
    batchEnd: 0,
    fill: 0,
    qoi: 0,
    zstd: 0,
    cacheHit: 0,
    videoRegion: 0,
    scrollCopy: 0,
    gridOffset: 0,
    scrollStats: 0,
    unknown: 0,
  };
  pendingTileBatch: PendingTileBatchStats = {
    hasScrollCopy: false,
    maxAbsDy: 0,
    gridOffsetY: 0,
    fill: 0,
    qoi: 0,
    cacheHit: 0,
    qoiBytes: 0,
  };
  scrollBatchStats: ScrollBatchStats = {
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
  };

  private readonly hostScrollHealthTracker: HostScrollHealthTracker;

  constructor(hostScrollHealthTracker: HostScrollHealthTracker = new HostScrollHealthTracker()) {
    this.hostScrollHealthTracker = hostScrollHealthTracker;
  }

  recordRx(channelId: number, frameBytes: number): void {
    this.rxBytes += frameBytes;
    this.rxFrames += 1;
    this.rxChannelBytes[channelId] = (this.rxChannelBytes[channelId] ?? 0) + frameBytes;
    this.rxChannelFrames[channelId] = (this.rxChannelFrames[channelId] ?? 0) + 1;
  }

  recordTx(channelId: number, frameBytes: number): void {
    this.txBytes += frameBytes;
    this.txFrames += 1;
    this.txChannelBytes[channelId] = (this.txChannelBytes[channelId] ?? 0) + frameBytes;
    this.txChannelFrames[channelId] = (this.txChannelFrames[channelId] ?? 0) + 1;
  }

  resetPendingTileBatch(): void {
    this.pendingTileBatch.hasScrollCopy = false;
    this.pendingTileBatch.maxAbsDy = 0;
    this.pendingTileBatch.gridOffsetY = 0;
    this.pendingTileBatch.fill = 0;
    this.pendingTileBatch.qoi = 0;
    this.pendingTileBatch.cacheHit = 0;
    this.pendingTileBatch.qoiBytes = 0;
  }

  finalizePendingTileBatch(tileSize: number, gridCols: number, gridRows: number): void {
    const batch = this.pendingTileBatch;
    if (!batch.hasScrollCopy) {
      this.resetPendingTileBatch();
      return;
    }

    const updates = batch.fill + batch.qoi + batch.cacheHit;
    const isSubTile = batch.maxAbsDy > 0 && batch.maxAbsDy < tileSize;
    const potentialRows = gridRows + (batch.gridOffsetY !== 0 ? 1 : 0);
    const potentialTiles = gridCols * potentialRows;
    const savedTiles = Math.max(0, potentialTiles - updates);

    this.scrollBatchStats.scrollBatches += 1;
    this.scrollBatchStats.scrollUpdateCommands += updates;
    this.scrollBatchStats.scrollQoiCommands += batch.qoi;
    this.scrollBatchStats.scrollCacheHitCommands += batch.cacheHit;
    this.scrollBatchStats.scrollFillCommands += batch.fill;
    this.scrollBatchStats.scrollQoiBytes += batch.qoiBytes;
    this.scrollBatchStats.scrollPotentialTiles += potentialTiles;
    this.scrollBatchStats.scrollSavedTiles += savedTiles;

    if (isSubTile) {
      this.scrollBatchStats.subTileScrollBatches += 1;
      this.scrollBatchStats.subTileScrollUpdateCommands += updates;
      this.scrollBatchStats.subTileScrollQoiCommands += batch.qoi;
      this.scrollBatchStats.subTileScrollQoiBytes += batch.qoiBytes;
      this.scrollBatchStats.subTileScrollPotentialTiles += potentialTiles;
      this.scrollBatchStats.subTileScrollSavedTiles += savedTiles;
    }

    this.resetPendingTileBatch();
  }

  recordHostScrollStats(
    hostScrollBatchesTotal: number,
    hostScrollFallbacksTotal: number,
    hostScrollPotentialTilesTotal: number,
    hostScrollSavedTilesTotal: number,
    hostScrollNonQuantizedFallbacksTotal: number,
    hostScrollResidualFullRepaintsTotal: number,
    hostScrollResidualInteriorLimitFallbacksTotal: number,
    hostScrollResidualLowSavedRatioFallbacksTotal: number,
    hostScrollResidualLargeRowShiftFallbacksTotal: number,
    hostScrollResidualOtherFallbacksTotal: number,
    hostScrollZeroSavedBatchesTotal: number,
    hostScrollSplitRegionBatchesTotal: number,
    hostScrollStickyBandBatchesTotal: number,
    hostScrollChromeTilesTotal: number,
    hostScrollExposedStripTilesTotal: number,
    hostScrollInteriorResidualTilesTotal: number,
    hostScrollEdgeStripResidualTilesTotal = 0,
    hostScrollSmallEdgeStripResidualTilesTotal = 0,
    hostScrollSmallEdgeStripResidualRowsTotal = 0,
    hostScrollSmallEdgeStripResidualAreaPxTotal = 0,
    hostSentHashEntries = 0,
    hostSentHashEvictionsTotal = 0,
    hostCacheMissReportsTotal = 0,
  ): void {
    this.hostScrollHealthTracker.record(
      hostScrollBatchesTotal,
      hostScrollFallbacksTotal,
      hostScrollPotentialTilesTotal,
      hostScrollSavedTilesTotal,
      hostScrollNonQuantizedFallbacksTotal,
      hostScrollResidualFullRepaintsTotal,
      hostScrollResidualInteriorLimitFallbacksTotal,
      hostScrollResidualLowSavedRatioFallbacksTotal,
      hostScrollResidualLargeRowShiftFallbacksTotal,
      hostScrollResidualOtherFallbacksTotal,
      hostScrollZeroSavedBatchesTotal,
      hostScrollSplitRegionBatchesTotal,
      hostScrollStickyBandBatchesTotal,
      hostScrollChromeTilesTotal,
      hostScrollExposedStripTilesTotal,
      hostScrollInteriorResidualTilesTotal,
      hostScrollEdgeStripResidualTilesTotal,
      hostScrollSmallEdgeStripResidualTilesTotal,
      hostScrollSmallEdgeStripResidualRowsTotal,
      hostScrollSmallEdgeStripResidualAreaPxTotal,
      hostSentHashEntries,
      hostSentHashEvictionsTotal,
      hostCacheMissReportsTotal,
    );
  }

  getSessionStats(
    tileCacheRuntime: TileCacheRuntimeStats,
    cameraTelemetry?: CameraTelemetrySnapshot,
  ): SessionStatsSnapshot {
    return SessionStatsSnapshotBuilder.build({
      nowMs: performance.now(),
      sessionStartAtMs: this.sessionStartAtMs,
      frameCount: this.frameCount,
      rxBytes: this.rxBytes,
      txBytes: this.txBytes,
      rxFrames: this.rxFrames,
      txFrames: this.txFrames,
      rxChannelBytes: this.rxChannelBytes,
      rxChannelFrames: this.rxChannelFrames,
      txChannelBytes: this.txChannelBytes,
      txChannelFrames: this.txChannelFrames,
      videoDatagramsRx: this.videoDatagramsRx,
      videoDatagramBytesRx: this.videoDatagramBytesRx,
      videoFramesDropped: this.videoFramesDropped,
      tileCommandBytes: this.tileCommandBytes,
      tileCommandCounts: this.tileCommandCounts,
      scrollBatchStats: this.scrollBatchStats,
      hostScrollHealth: this.hostScrollHealthTracker.snapshot(),
      tileCacheRuntime,
      cameraTelemetry,
    });
  }
}
