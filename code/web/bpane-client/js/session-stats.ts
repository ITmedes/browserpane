/**
 * SessionStats — extracted from BpaneSession.
 *
 * Pure data tracking with no side effects. Holds all transfer counters,
 * tile command counters, scroll batch stats, and host scroll health data.
 */

import {
  CH_VIDEO, CH_AUDIO_OUT, CH_AUDIO_IN, CH_VIDEO_IN, CH_INPUT, CH_CURSOR,
  CH_CLIPBOARD, CH_CONTROL,
} from './protocol.js';
import { CH_TILES } from './tile-compositor.js';
import type { CameraTelemetrySnapshot } from './camera-controller.js';

export interface ChannelTransferStats {
  bytes: number;
  frames: number;
}

export interface TileCommandStats {
  gridConfig: number;
  batchEnd: number;
  fill: number;
  qoi: number;
  zstd: number;
  cacheHit: number;
  videoRegion: number;
  scrollCopy: number;
  gridOffset: number;
  scrollStats: number;
  unknown: number;
}

export interface SessionStatsSnapshot {
  elapsedMs: number;
  transfer: {
    rxBytes: number;
    txBytes: number;
    rxFrames: number;
    txFrames: number;
    rxByChannel: Record<string, ChannelTransferStats>;
    txByChannel: Record<string, ChannelTransferStats>;
  };
  tiles: {
    commandBytes: number;
    commands: TileCommandStats;
    imageCommands: number;
    videoCommands: number;
    drawCommands: number;
    totalCommands: number;
    cacheHitsObserved: number;
    cacheMissesObserved: number;
    cacheHitRateObserved: number;
    cacheSizeObserved: number;
    redundantQoiCommands: number;
    redundantQoiBytes: number;
    scrollComposition: {
      scrollBatches: number;
      subTileScrollBatches: number;
      scrollUpdateCommands: number;
      scrollQoiCommands: number;
      scrollCacheHitCommands: number;
      scrollFillCommands: number;
      scrollQoiBytes: number;
      scrollPotentialTiles: number;
      scrollSavedTiles: number;
      scrollReuseRate: number;
      subTileScrollUpdateCommands: number;
      subTileScrollQoiCommands: number;
      subTileScrollQoiBytes: number;
      subTileScrollPotentialTiles: number;
      subTileScrollSavedTiles: number;
      subTileScrollReuseRate: number;
      subTileComposeCandidateRate: number;
    };
    scrollHealth: {
      hostScrollBatchesTotal: number;
      hostScrollFallbacksTotal: number;
      hostFallbackRate: number;
      hostFallbackRateRecent20: number;
      hostFallbackRateRecent50: number;
      hostFallbackRateRecent20Batches: number;
      hostFallbackRateRecent50Batches: number;
      hostScrollPotentialTilesTotal: number;
      hostScrollSavedTilesTotal: number;
      hostScrollSavedRate: number;
      lastHostScrollStatsAtMs: number;
    };
  };
  video: {
    decodedFrames: number;
    droppedFrames: number;
    datagrams: number;
    datagramBytes: number;
  };
  camera: {
    supported: boolean;
    active: boolean;
    profile: CameraTelemetrySnapshot['profile'];
    qualityLimitationReason: CameraTelemetrySnapshot['qualityLimitationReason'];
    transportBytesSent: number;
    transportFramesSent: number;
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
  };
}

export interface TileCacheRuntimeStats {
  hits: number;
  misses: number;
  hitRate: number;
  size: number;
  qoiRedundant: number;
  qoiRedundantBytes: number;
  zstdRedundant: number;
  zstdRedundantBytes: number;
}

export class SessionStats {
  private static readonly HOST_SCROLL_HISTORY_MAX_BATCHES = 256;

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
  pendingTileBatch = {
    hasScrollCopy: false,
    maxAbsDy: 0,
    gridOffsetY: 0,
    fill: 0,
    qoi: 0,
    cacheHit: 0,
    qoiBytes: 0,
  };
  scrollBatchStats = {
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
  hostScrollHealth = {
    hostScrollBatchesTotal: 0,
    hostScrollFallbacksTotal: 0,
    hostScrollPotentialTilesTotal: 0,
    hostScrollSavedTilesTotal: 0,
    lastHostScrollStatsAtMs: 0,
  };
  private hostScrollStatsInitialized = false;
  private hostScrollRecentHistory: Array<{ batches: number; fallbacks: number }> = [];

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
    const b = this.pendingTileBatch;
    if (!b.hasScrollCopy) {
      this.resetPendingTileBatch();
      return;
    }

    const updates = b.fill + b.qoi + b.cacheHit;
    const isSubTile = b.maxAbsDy > 0 && b.maxAbsDy < tileSize;
    const potentialRows = gridRows + (b.gridOffsetY !== 0 ? 1 : 0);
    const potentialTiles = gridCols * potentialRows;
    const savedTiles = Math.max(0, potentialTiles - updates);

    this.scrollBatchStats.scrollBatches += 1;
    this.scrollBatchStats.scrollUpdateCommands += updates;
    this.scrollBatchStats.scrollQoiCommands += b.qoi;
    this.scrollBatchStats.scrollCacheHitCommands += b.cacheHit;
    this.scrollBatchStats.scrollFillCommands += b.fill;
    this.scrollBatchStats.scrollQoiBytes += b.qoiBytes;
    this.scrollBatchStats.scrollPotentialTiles += potentialTiles;
    this.scrollBatchStats.scrollSavedTiles += savedTiles;

    if (isSubTile) {
      this.scrollBatchStats.subTileScrollBatches += 1;
      this.scrollBatchStats.subTileScrollUpdateCommands += updates;
      this.scrollBatchStats.subTileScrollQoiCommands += b.qoi;
      this.scrollBatchStats.subTileScrollQoiBytes += b.qoiBytes;
      this.scrollBatchStats.subTileScrollPotentialTiles += potentialTiles;
      this.scrollBatchStats.subTileScrollSavedTiles += savedTiles;
    }

    this.resetPendingTileBatch();
  }

  static channelLabel(channelId: number): string {
    switch (channelId) {
      case CH_VIDEO: return 'video';
      case CH_AUDIO_OUT: return 'audioOut';
      case CH_AUDIO_IN: return 'audioIn';
      case CH_VIDEO_IN: return 'videoIn';
      case CH_INPUT: return 'input';
      case CH_CURSOR: return 'cursor';
      case CH_CLIPBOARD: return 'clipboard';
      case CH_CONTROL: return 'control';
      case CH_TILES: return 'tiles';
      default: return `ch${channelId}`;
    }
  }

  snapshotChannelStats(
    bytesByChannel: Record<number, number>,
    framesByChannel: Record<number, number>,
  ): Record<string, ChannelTransferStats> {
    const ids = new Set<number>([
      CH_VIDEO,
      CH_AUDIO_OUT,
      CH_AUDIO_IN,
      CH_VIDEO_IN,
      CH_INPUT,
      CH_CURSOR,
      CH_CLIPBOARD,
      CH_CONTROL,
      CH_TILES,
      ...Object.keys(bytesByChannel).map((k) => Number(k)),
      ...Object.keys(framesByChannel).map((k) => Number(k)),
    ]);
    const out: Record<string, ChannelTransferStats> = {};
    for (const id of ids) {
      out[SessionStats.channelLabel(id)] = {
        bytes: bytesByChannel[id] ?? 0,
        frames: framesByChannel[id] ?? 0,
      };
    }
    return out;
  }

  recordHostScrollStats(
    hostScrollBatchesTotal: number,
    hostScrollFallbacksTotal: number,
    hostScrollPotentialTilesTotal: number,
    hostScrollSavedTilesTotal: number,
  ): void {
    const prevBatches = this.hostScrollHealth.hostScrollBatchesTotal;
    const prevFallbacks = this.hostScrollHealth.hostScrollFallbacksTotal;
    const prevPotential = this.hostScrollHealth.hostScrollPotentialTilesTotal;
    const prevSaved = this.hostScrollHealth.hostScrollSavedTilesTotal;

    if (
      !this.hostScrollStatsInitialized
      || hostScrollBatchesTotal < prevBatches
      || hostScrollFallbacksTotal < prevFallbacks
      || hostScrollPotentialTilesTotal < prevPotential
      || hostScrollSavedTilesTotal < prevSaved
    ) {
      this.hostScrollRecentHistory = [];
      this.hostScrollStatsInitialized = true;
    } else {
      const deltaBatches = hostScrollBatchesTotal - prevBatches;
      const deltaFallbacks = hostScrollFallbacksTotal - prevFallbacks;
      if (deltaBatches > 0 || deltaFallbacks > 0) {
        this.hostScrollRecentHistory.push({
          batches: Math.max(0, deltaBatches),
          fallbacks: Math.max(0, deltaFallbacks),
        });
        this.pruneHostScrollHistory();
      }
    }

    this.hostScrollHealth.hostScrollBatchesTotal = hostScrollBatchesTotal;
    this.hostScrollHealth.hostScrollFallbacksTotal = hostScrollFallbacksTotal;
    this.hostScrollHealth.hostScrollPotentialTilesTotal = hostScrollPotentialTilesTotal;
    this.hostScrollHealth.hostScrollSavedTilesTotal = hostScrollSavedTilesTotal;
    this.hostScrollHealth.lastHostScrollStatsAtMs = performance.now();
  }

  private pruneHostScrollHistory(): void {
    let retainedBatches = 0;
    let keepFrom = this.hostScrollRecentHistory.length;
    while (keepFrom > 0) {
      const sample = this.hostScrollRecentHistory[keepFrom - 1];
      retainedBatches += sample.batches;
      if (retainedBatches > SessionStats.HOST_SCROLL_HISTORY_MAX_BATCHES) {
        break;
      }
      keepFrom -= 1;
    }
    if (keepFrom > 0) {
      this.hostScrollRecentHistory = this.hostScrollRecentHistory.slice(keepFrom);
    }
  }

  private recentHostFallbackRate(windowBatches: number): { rate: number; batches: number } {
    let remaining = Math.max(0, windowBatches);
    let batches = 0;
    let fallbacks = 0;
    for (let i = this.hostScrollRecentHistory.length - 1; i >= 0 && remaining > 0; i -= 1) {
      const sample = this.hostScrollRecentHistory[i];
      if (sample.batches <= 0) continue;
      const take = Math.min(remaining, sample.batches);
      const fallbackShare = take === sample.batches
        ? sample.fallbacks
        : (sample.fallbacks * take) / sample.batches;
      batches += take;
      fallbacks += fallbackShare;
      remaining -= take;
    }
    return {
      rate: batches > 0 ? (fallbacks / batches) * 100 : 0,
      batches,
    };
  }

  getSessionStats(
    tileCacheRuntime: TileCacheRuntimeStats,
    cameraTelemetry?: CameraTelemetrySnapshot,
  ): SessionStatsSnapshot {
    const tile = this.tileCommandCounts;
    const scroll = this.scrollBatchStats;
    const totalCommands = tile.gridConfig
      + tile.batchEnd
      + tile.fill
      + tile.qoi
      + tile.zstd
      + tile.cacheHit
      + tile.videoRegion
      + tile.scrollCopy
      + tile.gridOffset
      + tile.scrollStats
      + tile.unknown;
    const scrollReuseRate = scroll.scrollPotentialTiles > 0
      ? (scroll.scrollSavedTiles / scroll.scrollPotentialTiles) * 100
      : 0;
    const subTileScrollReuseRate = scroll.subTileScrollPotentialTiles > 0
      ? (scroll.subTileScrollSavedTiles / scroll.subTileScrollPotentialTiles) * 100
      : 0;
    const subTileComposeCandidateRate = scroll.subTileScrollUpdateCommands > 0
      ? (scroll.subTileScrollQoiCommands / scroll.subTileScrollUpdateCommands) * 100
      : 0;
    const hostFallbackRate = this.hostScrollHealth.hostScrollBatchesTotal > 0
      ? (this.hostScrollHealth.hostScrollFallbacksTotal / this.hostScrollHealth.hostScrollBatchesTotal) * 100
      : 0;
    const hostFallbackRecent20 = this.recentHostFallbackRate(20);
    const hostFallbackRecent50 = this.recentHostFallbackRate(50);
    const hostScrollSavedRate = this.hostScrollHealth.hostScrollPotentialTilesTotal > 0
      ? (this.hostScrollHealth.hostScrollSavedTilesTotal / this.hostScrollHealth.hostScrollPotentialTilesTotal) * 100
      : 0;
    const camera = cameraTelemetry ?? {
      supported: false,
      active: false,
      profile: null,
      qualityLimitationReason: 'none' as const,
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
    };
    return {
      elapsedMs: Math.max(0, performance.now() - this.sessionStartAtMs),
      transfer: {
        rxBytes: this.rxBytes,
        txBytes: this.txBytes,
        rxFrames: this.rxFrames,
        txFrames: this.txFrames,
        rxByChannel: this.snapshotChannelStats(this.rxChannelBytes, this.rxChannelFrames),
        txByChannel: this.snapshotChannelStats(this.txChannelBytes, this.txChannelFrames),
      },
      tiles: {
        commandBytes: this.tileCommandBytes,
        commands: { ...tile },
        imageCommands: tile.qoi + tile.zstd,
        videoCommands: tile.videoRegion,
        drawCommands: tile.fill + tile.qoi + tile.zstd + tile.cacheHit + tile.scrollCopy + tile.gridOffset,
        totalCommands,
        cacheHitsObserved: tileCacheRuntime.hits,
        cacheMissesObserved: tileCacheRuntime.misses,
        cacheHitRateObserved: tileCacheRuntime.hitRate,
        cacheSizeObserved: tileCacheRuntime.size,
        redundantQoiCommands: tileCacheRuntime.qoiRedundant + tileCacheRuntime.zstdRedundant,
        redundantQoiBytes: tileCacheRuntime.qoiRedundantBytes + tileCacheRuntime.zstdRedundantBytes,
        scrollComposition: {
          scrollBatches: scroll.scrollBatches,
          subTileScrollBatches: scroll.subTileScrollBatches,
          scrollUpdateCommands: scroll.scrollUpdateCommands,
          scrollQoiCommands: scroll.scrollQoiCommands,
          scrollCacheHitCommands: scroll.scrollCacheHitCommands,
          scrollFillCommands: scroll.scrollFillCommands,
          scrollQoiBytes: scroll.scrollQoiBytes,
          scrollPotentialTiles: scroll.scrollPotentialTiles,
          scrollSavedTiles: scroll.scrollSavedTiles,
          scrollReuseRate,
          subTileScrollUpdateCommands: scroll.subTileScrollUpdateCommands,
          subTileScrollQoiCommands: scroll.subTileScrollQoiCommands,
          subTileScrollQoiBytes: scroll.subTileScrollQoiBytes,
          subTileScrollPotentialTiles: scroll.subTileScrollPotentialTiles,
          subTileScrollSavedTiles: scroll.subTileScrollSavedTiles,
          subTileScrollReuseRate,
          subTileComposeCandidateRate,
        },
        scrollHealth: {
          hostScrollBatchesTotal: this.hostScrollHealth.hostScrollBatchesTotal,
          hostScrollFallbacksTotal: this.hostScrollHealth.hostScrollFallbacksTotal,
          hostFallbackRate,
          hostFallbackRateRecent20: hostFallbackRecent20.rate,
          hostFallbackRateRecent50: hostFallbackRecent50.rate,
          hostFallbackRateRecent20Batches: hostFallbackRecent20.batches,
          hostFallbackRateRecent50Batches: hostFallbackRecent50.batches,
          hostScrollPotentialTilesTotal: this.hostScrollHealth.hostScrollPotentialTilesTotal,
          hostScrollSavedTilesTotal: this.hostScrollHealth.hostScrollSavedTilesTotal,
          hostScrollSavedRate,
          lastHostScrollStatsAtMs: this.hostScrollHealth.lastHostScrollStatsAtMs,
        },
      },
      video: {
        decodedFrames: this.frameCount,
        droppedFrames: this.videoFramesDropped,
        datagrams: this.videoDatagramsRx,
        datagramBytes: this.videoDatagramBytesRx,
      },
      camera: {
        supported: camera.supported,
        active: camera.active,
        profile: camera.profile,
        qualityLimitationReason: camera.qualityLimitationReason,
        transportBytesSent: this.txChannelBytes[CH_VIDEO_IN] ?? 0,
        transportFramesSent: this.txChannelFrames[CH_VIDEO_IN] ?? 0,
        framesCaptured: camera.framesCaptured,
        framesEncoded: camera.framesEncoded,
        keyframesEncoded: camera.keyframesEncoded,
        encodedBytes: camera.encodedBytes,
        transportFramesQueued: camera.transportFramesQueued,
        transportFramesReplaced: camera.transportFramesReplaced,
        encoderQueueDrops: camera.encoderQueueDrops,
        averageEncodeTimeMs: camera.averageEncodeTimeMs,
        maxEncodeTimeMs: camera.maxEncodeTimeMs,
        profileUpgrades: camera.profileUpgrades,
        profileDowngrades: camera.profileDowngrades,
        reconfigurations: camera.reconfigurations,
      },
    };
  }
}
