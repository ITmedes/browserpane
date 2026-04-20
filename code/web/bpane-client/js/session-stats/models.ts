import type { CameraTelemetrySnapshot } from '../camera-controller.js';

export type ChannelTransferStats = {
  bytes: number;
  frames: number;
};

export type TileCommandStats = {
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
};

export type PendingTileBatchStats = {
  hasScrollCopy: boolean;
  maxAbsDy: number;
  gridOffsetY: number;
  fill: number;
  qoi: number;
  cacheHit: number;
  qoiBytes: number;
};

export type ScrollBatchStats = {
  scrollBatches: number;
  subTileScrollBatches: number;
  scrollUpdateCommands: number;
  scrollQoiCommands: number;
  scrollCacheHitCommands: number;
  scrollFillCommands: number;
  scrollQoiBytes: number;
  scrollPotentialTiles: number;
  scrollSavedTiles: number;
  subTileScrollUpdateCommands: number;
  subTileScrollQoiCommands: number;
  subTileScrollQoiBytes: number;
  subTileScrollPotentialTiles: number;
  subTileScrollSavedTiles: number;
};

export type HostScrollHealthSnapshot = {
  hostScrollBatchesTotal: number;
  hostScrollFallbacksTotal: number;
  hostScrollNonQuantizedFallbacksTotal: number;
  hostScrollResidualFullRepaintsTotal: number;
  hostScrollZeroSavedBatchesTotal: number;
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

export type SessionStatsSnapshot = {
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
    scrollHealth: HostScrollHealthSnapshot;
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
};

export type TileCacheRuntimeStats = {
  hits: number;
  misses: number;
  hitRate: number;
  size: number;
  qoiRedundant: number;
  qoiRedundantBytes: number;
  zstdRedundant: number;
  zstdRedundantBytes: number;
};
