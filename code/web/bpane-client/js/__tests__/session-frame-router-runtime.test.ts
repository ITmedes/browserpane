import { describe, expect, it, vi } from 'vitest';

import {
  CH_AUDIO_OUT,
  CH_CLIPBOARD,
  CH_CONTROL,
  CH_CURSOR,
  CH_FILE_DOWN,
  CH_VIDEO,
} from '../protocol.js';
import { CH_TILES } from '../tile-compositor.js';
import { SessionFrameRouterRuntime } from '../session-frame-router-runtime.js';
import type { TileCommandStats } from '../session-stats.js';
import type { PendingTileBatchStats } from '../session-stats/models.js';
import type { TileCommand, TileGridConfig } from '../tile-cache.js';

function createTileCommandCounts(): TileCommandStats {
  return {
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
}

function createPendingTileBatch(): PendingTileBatchStats {
  return {
    hasScrollCopy: false,
    maxAbsDy: 0,
    gridOffsetY: 0,
    fill: 0,
    qoi: 0,
    cacheHit: 0,
    qoiBytes: 0,
  };
}

describe('SessionFrameRouterRuntime', () => {
  it('routes non-tile channels to their handlers', () => {
    const stats = {
      tileCommandBytes: 0,
      tileCommandCounts: createTileCommandCounts(),
      pendingTileBatch: createPendingTileBatch(),
      resetPendingTileBatch: vi.fn(),
      finalizePendingTileBatch: vi.fn(),
      recordHostScrollStats: vi.fn(),
    };
    const handleVideoFrame = vi.fn();
    const handleAudioFrame = vi.fn();
    const handleCursorUpdate = vi.fn();
    const handleClipboardUpdate = vi.fn();
    const handleControlMessage = vi.fn();
    const handleFileDownloadFrame = vi.fn();
    const runtime = new SessionFrameRouterRuntime({
      tileCompositor: {
        handlePayload: vi.fn(),
        getGridConfig: vi.fn(),
      },
      stats,
      handleVideoFrame,
      handleAudioFrame,
      handleCursorUpdate,
      handleClipboardUpdate,
      handleControlMessage,
      handleFileDownloadFrame,
      clearVideoOverlay: vi.fn(),
      markDisplayDirty: vi.fn(),
    });

    const payload = new Uint8Array([1, 2, 3]);
    runtime.handleFrame(CH_VIDEO, payload);
    runtime.handleFrame(CH_AUDIO_OUT, payload);
    runtime.handleFrame(CH_CURSOR, payload);
    runtime.handleFrame(CH_CLIPBOARD, payload);
    runtime.handleFrame(CH_CONTROL, payload);
    runtime.handleFrame(CH_FILE_DOWN, payload);

    expect(handleVideoFrame).toHaveBeenCalledWith(payload);
    expect(handleAudioFrame).toHaveBeenCalledWith(payload);
    expect(handleCursorUpdate).toHaveBeenCalledWith(payload);
    expect(handleClipboardUpdate).toHaveBeenCalledWith(payload);
    expect(handleControlMessage).toHaveBeenCalledWith(payload);
    expect(handleFileDownloadFrame).toHaveBeenCalledWith(payload);
  });

  it('counts malformed tile payloads as unknown commands', () => {
    const stats = {
      tileCommandBytes: 0,
      tileCommandCounts: createTileCommandCounts(),
      pendingTileBatch: createPendingTileBatch(),
      resetPendingTileBatch: vi.fn(),
      finalizePendingTileBatch: vi.fn(),
      recordHostScrollStats: vi.fn(),
    };
    const runtime = new SessionFrameRouterRuntime({
      tileCompositor: {
        handlePayload: vi.fn(() => null),
        getGridConfig: vi.fn(),
      },
      stats,
      handleVideoFrame: vi.fn(),
      handleAudioFrame: vi.fn(),
      handleCursorUpdate: vi.fn(),
      handleClipboardUpdate: vi.fn(),
      handleControlMessage: vi.fn(),
      handleFileDownloadFrame: vi.fn(),
      clearVideoOverlay: vi.fn(),
      markDisplayDirty: vi.fn(),
    });

    runtime.handleFrame(CH_TILES, new Uint8Array([0x01, 0x02, 0x03]));

    expect(stats.tileCommandBytes).toBe(3);
    expect(stats.tileCommandCounts.unknown).toBe(1);
  });

  it('resets pending tile batch and clears the overlay on grid-config', () => {
    const stats = {
      tileCommandBytes: 0,
      tileCommandCounts: createTileCommandCounts(),
      pendingTileBatch: {
        ...createPendingTileBatch(),
        fill: 2,
        qoi: 3,
        cacheHit: 4,
      },
      resetPendingTileBatch: vi.fn(function resetPendingTileBatch(this: typeof stats) {
        this.pendingTileBatch = createPendingTileBatch();
      }),
      finalizePendingTileBatch: vi.fn(),
      recordHostScrollStats: vi.fn(),
    };
    const gridConfigCommand: TileCommand = {
      type: 'grid-config',
      config: {
        tileSize: 64,
        cols: 20,
        rows: 10,
        screenW: 1280,
        screenH: 720,
      },
    };
    const clearVideoOverlay = vi.fn();
    const runtime = new SessionFrameRouterRuntime({
      tileCompositor: {
        handlePayload: vi.fn(() => gridConfigCommand),
        getGridConfig: vi.fn(),
      },
      stats,
      handleVideoFrame: vi.fn(),
      handleAudioFrame: vi.fn(),
      handleCursorUpdate: vi.fn(),
      handleClipboardUpdate: vi.fn(),
      handleControlMessage: vi.fn(),
      handleFileDownloadFrame: vi.fn(),
      clearVideoOverlay,
      markDisplayDirty: vi.fn(),
    });

    runtime.handleFrame(CH_TILES, new Uint8Array([0x01]));

    expect(stats.tileCommandCounts.gridConfig).toBe(1);
    expect(stats.resetPendingTileBatch).toHaveBeenCalledOnce();
    expect(stats.pendingTileBatch).toEqual(createPendingTileBatch());
    expect(clearVideoOverlay).toHaveBeenCalledOnce();
  });

  it('finalizes batch stats and marks display dirty on batch-end', () => {
    const stats = {
      tileCommandBytes: 0,
      tileCommandCounts: createTileCommandCounts(),
      pendingTileBatch: {
        ...createPendingTileBatch(),
        hasScrollCopy: true,
        fill: 1,
      },
      resetPendingTileBatch: vi.fn(),
      finalizePendingTileBatch: vi.fn(),
      recordHostScrollStats: vi.fn(),
    };
    const batchEndCommand: TileCommand = {
      type: 'batch-end',
      frameSeq: 17,
    };
    const markDisplayDirty = vi.fn();
    const runtime = new SessionFrameRouterRuntime({
      tileCompositor: {
        handlePayload: vi.fn(() => batchEndCommand),
        getGridConfig: vi.fn(() => ({
          tileSize: 64,
          cols: 20,
          rows: 10,
          screenW: 1280,
          screenH: 720,
        })),
      },
      stats,
      handleVideoFrame: vi.fn(),
      handleAudioFrame: vi.fn(),
      handleCursorUpdate: vi.fn(),
      handleClipboardUpdate: vi.fn(),
      handleControlMessage: vi.fn(),
      handleFileDownloadFrame: vi.fn(),
      clearVideoOverlay: vi.fn(),
      markDisplayDirty,
    });

    runtime.handleFrame(CH_TILES, new Uint8Array([0x06]));

    expect(stats.tileCommandCounts.batchEnd).toBe(1);
    expect(stats.finalizePendingTileBatch).toHaveBeenCalledWith(64, 20, 10);
    expect(markDisplayDirty).toHaveBeenCalledOnce();
  });

  it('updates scroll-related tile stats and overlay clearing rules', () => {
    const stats = {
      tileCommandBytes: 0,
      tileCommandCounts: createTileCommandCounts(),
      pendingTileBatch: createPendingTileBatch(),
      resetPendingTileBatch: vi.fn(),
      finalizePendingTileBatch: vi.fn(),
      recordHostScrollStats: vi.fn(),
    };
    const clearVideoOverlay = vi.fn();
    const handlePayload = vi.fn()
      .mockReturnValueOnce({
        type: 'scroll-copy',
        dx: 0,
        dy: -96,
        regionTop: 0,
        regionBottom: 720,
        regionRight: 1280,
      })
      .mockReturnValueOnce({
        type: 'grid-offset',
        offsetX: 0,
        offsetY: 0,
      })
      .mockReturnValueOnce({
        type: 'grid-offset',
        offsetX: 0,
        offsetY: 12,
      })
      .mockReturnValueOnce({
        type: 'scroll-stats',
        scrollBatchesTotal: 10,
        scrollFullFallbacksTotal: 2,
        scrollPotentialTilesTotal: 100,
        scrollSavedTilesTotal: 80,
        scrollNonQuantizedFallbacksTotal: 1,
        scrollResidualFullRepaintsTotal: 1,
        scrollZeroSavedBatchesTotal: 3,
        hostSentHashEntries: 64,
        hostSentHashEvictionsTotal: 5,
        hostCacheMissReportsTotal: 4,
      });
    const runtime = new SessionFrameRouterRuntime({
      tileCompositor: {
        handlePayload,
        getGridConfig: vi.fn(),
      },
      stats,
      handleVideoFrame: vi.fn(),
      handleAudioFrame: vi.fn(),
      handleCursorUpdate: vi.fn(),
      handleClipboardUpdate: vi.fn(),
      handleControlMessage: vi.fn(),
      handleFileDownloadFrame: vi.fn(),
      clearVideoOverlay,
      markDisplayDirty: vi.fn(),
    });

    runtime.handleFrame(CH_TILES, new Uint8Array([0x07]));
    runtime.handleFrame(CH_TILES, new Uint8Array([0x08]));
    runtime.handleFrame(CH_TILES, new Uint8Array([0x08]));
    runtime.handleFrame(CH_TILES, new Uint8Array([0x0a]));

    expect(stats.tileCommandCounts.scrollCopy).toBe(1);
    expect(stats.pendingTileBatch.hasScrollCopy).toBe(true);
    expect(stats.pendingTileBatch.maxAbsDy).toBe(96);
    expect(stats.tileCommandCounts.gridOffset).toBe(2);
    expect(stats.pendingTileBatch.gridOffsetY).toBe(12);
    expect(stats.tileCommandCounts.scrollStats).toBe(1);
    expect(stats.recordHostScrollStats).toHaveBeenCalledWith(10, 2, 100, 80, 1, 1, 3, 64, 5, 4);
    expect(clearVideoOverlay).toHaveBeenCalledTimes(2);
  });
});
