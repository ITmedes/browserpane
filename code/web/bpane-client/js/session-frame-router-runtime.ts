import {
  CH_AUDIO_OUT,
  CH_CLIPBOARD,
  CH_CONTROL,
  CH_CURSOR,
  CH_FILE_DOWN,
  CH_VIDEO,
} from './protocol.js';
import { CH_TILES, type TileCompositor } from './tile-compositor.js';
import type { TileCommandStats } from './session-stats.js';
import type { PendingTileBatchStats } from './session-stats/models.js';

type FrameRouterStats = {
  tileCommandBytes: number;
  tileCommandCounts: TileCommandStats;
  pendingTileBatch: PendingTileBatchStats;
  resetPendingTileBatch: () => void;
  finalizePendingTileBatch: (tileSize: number, gridCols: number, gridRows: number) => void;
  recordHostScrollStats: (
    hostScrollBatchesTotal: number,
    hostScrollFallbacksTotal: number,
    hostScrollPotentialTilesTotal: number,
    hostScrollSavedTilesTotal: number,
  ) => void;
};

export interface SessionFrameRouterRuntimeInput {
  tileCompositor: Pick<TileCompositor, 'handlePayload' | 'getGridConfig'>;
  stats: FrameRouterStats;
  handleVideoFrame: (payload: Uint8Array) => void;
  handleAudioFrame: (payload: Uint8Array) => void;
  handleCursorUpdate: (payload: Uint8Array) => void;
  handleClipboardUpdate: (payload: Uint8Array) => void;
  handleControlMessage: (payload: Uint8Array) => void;
  handleFileDownloadFrame: (payload: Uint8Array) => void;
  clearVideoOverlay: () => void;
  markDisplayDirty: () => void;
}

export class SessionFrameRouterRuntime {
  private readonly tileCompositor: Pick<TileCompositor, 'handlePayload' | 'getGridConfig'>;
  private readonly stats: FrameRouterStats;
  private readonly handleVideoFrame: (payload: Uint8Array) => void;
  private readonly handleAudioFrame: (payload: Uint8Array) => void;
  private readonly handleCursorUpdate: (payload: Uint8Array) => void;
  private readonly handleClipboardUpdate: (payload: Uint8Array) => void;
  private readonly handleControlMessage: (payload: Uint8Array) => void;
  private readonly handleFileDownloadFrame: (payload: Uint8Array) => void;
  private readonly clearVideoOverlay: () => void;
  private readonly markDisplayDirty: () => void;

  constructor(input: SessionFrameRouterRuntimeInput) {
    this.tileCompositor = input.tileCompositor;
    this.stats = input.stats;
    this.handleVideoFrame = input.handleVideoFrame;
    this.handleAudioFrame = input.handleAudioFrame;
    this.handleCursorUpdate = input.handleCursorUpdate;
    this.handleClipboardUpdate = input.handleClipboardUpdate;
    this.handleControlMessage = input.handleControlMessage;
    this.handleFileDownloadFrame = input.handleFileDownloadFrame;
    this.clearVideoOverlay = input.clearVideoOverlay;
    this.markDisplayDirty = input.markDisplayDirty;
  }

  handleFrame(channelId: number, payload: Uint8Array): void {
    switch (channelId) {
      case CH_VIDEO:
        this.handleVideoFrame(payload);
        break;
      case CH_AUDIO_OUT:
        this.handleAudioFrame(payload);
        break;
      case CH_CURSOR:
        this.handleCursorUpdate(payload);
        break;
      case CH_CLIPBOARD:
        this.handleClipboardUpdate(payload);
        break;
      case CH_CONTROL:
        this.handleControlMessage(payload);
        break;
      case CH_FILE_DOWN:
        this.handleFileDownloadFrame(payload);
        break;
      case CH_TILES:
        this.handleTilePayload(payload);
        break;
    }
  }

  private handleTilePayload(payload: Uint8Array): void {
    this.stats.tileCommandBytes += payload.byteLength;
    const command = this.tileCompositor.handlePayload(payload);
    if (!command) {
      this.stats.tileCommandCounts.unknown += 1;
      return;
    }

    switch (command.type) {
      case 'grid-config':
        this.stats.tileCommandCounts.gridConfig += 1;
        this.stats.resetPendingTileBatch();
        this.clearVideoOverlay();
        break;
      case 'batch-end': {
        this.stats.tileCommandCounts.batchEnd += 1;
        const grid = this.tileCompositor.getGridConfig();
        const tileSize = grid?.tileSize ?? 64;
        const cols = grid?.cols ?? 0;
        const rows = grid?.rows ?? 0;
        this.stats.finalizePendingTileBatch(tileSize, cols, rows);
        this.markDisplayDirty();
        break;
      }
      case 'fill':
        this.stats.tileCommandCounts.fill += 1;
        this.stats.pendingTileBatch.fill += 1;
        break;
      case 'qoi':
        this.stats.tileCommandCounts.qoi += 1;
        this.stats.pendingTileBatch.qoi += 1;
        this.stats.pendingTileBatch.qoiBytes += command.data.byteLength;
        break;
      case 'zstd':
        this.stats.tileCommandCounts.zstd += 1;
        this.stats.pendingTileBatch.qoi += 1;
        this.stats.pendingTileBatch.qoiBytes += command.data.byteLength;
        break;
      case 'cache-hit':
        this.stats.tileCommandCounts.cacheHit += 1;
        this.stats.pendingTileBatch.cacheHit += 1;
        break;
      case 'video-region':
        this.stats.tileCommandCounts.videoRegion += 1;
        break;
      case 'scroll-copy':
        this.stats.tileCommandCounts.scrollCopy += 1;
        this.stats.pendingTileBatch.hasScrollCopy = true;
        this.stats.pendingTileBatch.maxAbsDy = Math.max(
          this.stats.pendingTileBatch.maxAbsDy,
          Math.abs(command.dy),
        );
        this.clearVideoOverlay();
        break;
      case 'grid-offset':
        this.stats.tileCommandCounts.gridOffset += 1;
        this.stats.pendingTileBatch.gridOffsetY = command.offsetY;
        if (command.offsetX !== 0 || command.offsetY !== 0) {
          this.clearVideoOverlay();
        }
        break;
      case 'scroll-stats':
        this.stats.tileCommandCounts.scrollStats += 1;
        this.stats.recordHostScrollStats(
          command.scrollBatchesTotal,
          command.scrollFullFallbacksTotal,
          command.scrollPotentialTilesTotal,
          command.scrollSavedTilesTotal,
        );
        break;
      default:
        this.stats.tileCommandCounts.unknown += 1;
        break;
    }
  }
}
