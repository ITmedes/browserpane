import { CH_VIDEO_IN } from '../protocol.js';
import type { CameraTelemetrySnapshot } from '../camera-controller.js';
import { ChannelTransferStatsSnapshotBuilder } from './channel-transfer-stats-snapshot-builder.js';
import type {
  HostScrollHealthSnapshot,
  ScrollBatchStats,
  SessionStatsSnapshot,
  TileCacheRuntimeStats,
  TileCommandStats,
} from './models.js';

export type SessionStatsSnapshotBuildInput = {
  nowMs: number;
  sessionStartAtMs: number;
  frameCount: number;
  rxBytes: number;
  txBytes: number;
  rxFrames: number;
  txFrames: number;
  rxChannelBytes: Record<number, number>;
  rxChannelFrames: Record<number, number>;
  txChannelBytes: Record<number, number>;
  txChannelFrames: Record<number, number>;
  videoDatagramsRx: number;
  videoDatagramBytesRx: number;
  videoFramesDropped: number;
  tileCommandBytes: number;
  tileCommandCounts: TileCommandStats;
  scrollBatchStats: ScrollBatchStats;
  hostScrollHealth: HostScrollHealthSnapshot;
  tileCacheRuntime: TileCacheRuntimeStats;
  cameraTelemetry?: CameraTelemetrySnapshot;
};

export class SessionStatsSnapshotBuilder {
  static build(input: SessionStatsSnapshotBuildInput): SessionStatsSnapshot {
    const tile = input.tileCommandCounts;
    const scroll = input.scrollBatchStats;
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
    const camera = input.cameraTelemetry ?? SessionStatsSnapshotBuilder.defaultCameraTelemetry();

    return {
      elapsedMs: Math.max(0, input.nowMs - input.sessionStartAtMs),
      transfer: {
        rxBytes: input.rxBytes,
        txBytes: input.txBytes,
        rxFrames: input.rxFrames,
        txFrames: input.txFrames,
        rxByChannel: ChannelTransferStatsSnapshotBuilder.build(
          input.rxChannelBytes,
          input.rxChannelFrames,
        ),
        txByChannel: ChannelTransferStatsSnapshotBuilder.build(
          input.txChannelBytes,
          input.txChannelFrames,
        ),
      },
      tiles: {
        commandBytes: input.tileCommandBytes,
        commands: { ...tile },
        imageCommands: tile.qoi + tile.zstd,
        videoCommands: tile.videoRegion,
        drawCommands: tile.fill + tile.qoi + tile.zstd + tile.cacheHit + tile.scrollCopy + tile.gridOffset,
        totalCommands,
        cacheHitsObserved: input.tileCacheRuntime.hits,
        cacheMissesObserved: input.tileCacheRuntime.misses,
        cacheHitRateObserved: input.tileCacheRuntime.hitRate,
        cacheSizeObserved: input.tileCacheRuntime.size,
        redundantQoiCommands: input.tileCacheRuntime.qoiRedundant + input.tileCacheRuntime.zstdRedundant,
        redundantQoiBytes: input.tileCacheRuntime.qoiRedundantBytes + input.tileCacheRuntime.zstdRedundantBytes,
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
        scrollHealth: { ...input.hostScrollHealth },
      },
      video: {
        decodedFrames: input.frameCount,
        droppedFrames: input.videoFramesDropped,
        datagrams: input.videoDatagramsRx,
        datagramBytes: input.videoDatagramBytesRx,
      },
      camera: {
        supported: camera.supported,
        active: camera.active,
        profile: camera.profile,
        qualityLimitationReason: camera.qualityLimitationReason,
        transportBytesSent: input.txChannelBytes[CH_VIDEO_IN] ?? 0,
        transportFramesSent: input.txChannelFrames[CH_VIDEO_IN] ?? 0,
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

  private static defaultCameraTelemetry(): CameraTelemetrySnapshot {
    return {
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
    };
  }
}
