/**
 * Client-side tile compositor.
 *
 * Processes TileCommands from the server and composites them onto a canvas:
 * - Fill commands: draw solid-color rectangles
 * - Cache hits: draw previously cached decoded tiles
 * - QOI tiles: decode QOI data, cache decoded pixels, draw immediately
 * - Video region: tracked for the H.264 decoder to composite at the right position
 * - Batch end: signals that all tiles for a frame have been received
 */

import { TileCache, parseTileMessage, CH_TILES } from './tile-cache.js';
import type { TileCommand, TileGridConfig } from './tile-cache.js';
import { CacheHitTileRenderer } from './render/cache-hit-tile-renderer.js';
import { CanvasScrollCopyRenderer } from './render/canvas-scroll-copy-renderer.js';
import { FillTileRenderer } from './render/fill-tile-renderer.js';
import { QoiTileRenderer } from './render/qoi-tile-renderer.js';
import { TileBatchCommandApplier } from './render/tile-batch-command-applier.js';
import { TileBatchSequencer, type QueuedTileBatch } from './render/tile-batch-sequencer.js';
import {
  TileDrawRuntime,
  type CacheMissEvent,
  type TileCompositorRenderStats,
} from './render/tile-draw-runtime.js';
import { ZstdTileRenderer } from './render/zstd-tile-renderer.js';
import type { WebGLTileRenderer } from './webgl-compositor.js';

export interface CompositorStats extends TileCompositorRenderStats {
  batchesProcessed: number;
  batchesQueued: number;
  totalBatchCommands: number;
  maxBatchCommands: number;
  lastBatchCommands: number;
  currentPendingCommands: number;
  pendingCommandsHighWaterMark: number;
}

export class TileCompositor {
  private videoRegion: { x: number; y: number; w: number; h: number } | null = null;
  private pendingCommands: TileCommand[] = [];
  private readonly tileBatchCommandApplier: TileBatchCommandApplier;
  private readonly tileBatchSequencer = new TileBatchSequencer();
  private readonly tileDrawRuntime: TileDrawRuntime;

  stats: CompositorStats = {
    fills: 0,
    cacheHits: 0,
    cacheMisses: 0,
    qoiDecodes: 0,
    qoiRedundant: 0,
    qoiRedundantBytes: 0,
    zstdDecodes: 0,
    zstdRedundant: 0,
    zstdRedundantBytes: 0,
    batchesProcessed: 0,
    scrollCopies: 0,
    batchesQueued: 0,
    totalBatchCommands: 0,
    maxBatchCommands: 0,
    lastBatchCommands: 0,
    currentPendingCommands: 0,
    pendingCommandsHighWaterMark: 0,
  };

  constructor(
    cache?: TileCache,
    fillTileRenderer: FillTileRenderer = new FillTileRenderer(),
    cacheHitTileRenderer: CacheHitTileRenderer = new CacheHitTileRenderer(),
    qoiTileRenderer: QoiTileRenderer = new QoiTileRenderer(),
    zstdTileRenderer: ZstdTileRenderer = new ZstdTileRenderer(),
    canvasScrollCopyRenderer: CanvasScrollCopyRenderer = new CanvasScrollCopyRenderer(),
  ) {
    this.tileDrawRuntime = new TileDrawRuntime({
      cache,
      stats: this.stats,
      fillTileRenderer,
      cacheHitTileRenderer,
      qoiTileRenderer,
      zstdTileRenderer,
      canvasScrollCopyRenderer,
    });
    this.tileBatchCommandApplier = new TileBatchCommandApplier({
      applyScrollCopy: (dx, dy, regionTop, regionBottom, regionRight) => {
        this.tileDrawRuntime.applyScrollCopy(dx, dy, regionTop, regionBottom, regionRight);
      },
      setGridOffset: (offsetX, offsetY) => {
        this.tileDrawRuntime.setGridOffset(offsetX, offsetY);
      },
      setApplyOffsetMode: (applyOffset) => {
        this.tileDrawRuntime.setApplyOffsetMode(applyOffset);
      },
      setVideoRegion: (region) => {
        this.videoRegion = region;
      },
      drawFill: (col, row, rgba) => {
        this.tileDrawRuntime.drawFill(col, row, rgba);
      },
      drawCacheHit: (col, row, hash, frameSeq) => {
        this.tileDrawRuntime.drawCacheHit(col, row, hash, frameSeq);
      },
      drawQoi: (col, row, hash, data, epoch) => {
        this.tileDrawRuntime.drawQoi(col, row, hash, data, () => this.tileBatchSequencer.isCurrentEpoch(epoch));
      },
      drawZstd: (col, row, hash, data, epoch) => {
        this.tileDrawRuntime.drawZstd(col, row, hash, data, () => this.tileBatchSequencer.isCurrentEpoch(epoch));
      },
    });
  }

  /** Bind to a canvas rendering context for drawing (Canvas2D fallback). */
  setContext(ctx: CanvasRenderingContext2D): void {
    this.tileDrawRuntime.setContext(ctx);
  }

  /** Bind a WebGL2 renderer for GPU-accelerated drawing. When set, draw calls
   *  are routed to WebGL instead of Canvas2D. Pass null to revert to Canvas2D. */
  setWebGLRenderer(renderer: WebGLTileRenderer | null): void {
    this.tileDrawRuntime.setWebGLRenderer(renderer);
  }

  /** Whether a WebGL renderer is currently active. */
  hasWebGL(): boolean {
    return this.tileDrawRuntime.hasWebGL();
  }

  /** Current tile grid configuration, or null if not yet received. */
  getGridConfig(): TileGridConfig | null {
    return this.tileDrawRuntime.getGridConfig();
  }

  /** Current video region bounding box, or null. */
  getVideoRegion(): { x: number; y: number; w: number; h: number } | null {
    return this.videoRegion;
  }

  /** Access the underlying tile cache. */
  getCache(): TileCache {
    return this.tileDrawRuntime.getCache();
  }

  /** Register a callback for cache-miss telemetry back to the host. */
  setCacheMissHandler(handler: ((event: CacheMissEvent) => void) | null): void {
    this.tileDrawRuntime.setCacheMissHandler(handler);
  }

  /** Diagnostic switch to disable retained scroll-copy reuse. */
  setScrollCopyEnabled(enabled: boolean): void {
    this.tileDrawRuntime.setScrollCopyEnabled(enabled);
  }

  /**
   * Process a raw Tiles channel payload.
   * Returns the parsed command, or null if malformed.
   */
  handlePayload(payload: Uint8Array): TileCommand | null {
    const cmd = parseTileMessage(payload);
    if (!cmd) return null;
    this.processCommand(cmd);
    return cmd;
  }

  /** Process a parsed tile command. */
  processCommand(cmd: TileCommand): void {
    switch (cmd.type) {
      case 'grid-config':
        // Invalidate any in-flight async decode work and reset drawing state.
        this.tileBatchSequencer.invalidate();
        this.pendingCommands = [];
        this.stats.currentPendingCommands = 0;
        this.videoRegion = null;
        this.tileDrawRuntime.applyGridConfig(cmd.config);
        break;

      case 'batch-end':
        // Frame-sequenced, serialized batch processing.
        this.stats.batchesQueued += 1;
        this.stats.lastBatchCommands = this.pendingCommands.length;
        this.stats.totalBatchCommands += this.pendingCommands.length;
        this.stats.maxBatchCommands = Math.max(this.stats.maxBatchCommands, this.pendingCommands.length);
        this.tileBatchSequencer.enqueueOwnedBatch(
          cmd.frameSeq >>> 0,
          this.pendingCommands,
          (batch) => this.applyBatch(batch),
        );
        this.pendingCommands = [];
        this.stats.currentPendingCommands = 0;
        this.stats.batchesProcessed++;
        break;

      case 'scroll-stats':
        // Telemetry-only sideband command (handled by session stats layer).
        break;

      default:
        this.pendingCommands.push(cmd);
        this.stats.currentPendingCommands = this.pendingCommands.length;
        this.stats.pendingCommandsHighWaterMark = Math.max(
          this.stats.pendingCommandsHighWaterMark,
          this.pendingCommands.length,
        );
        break;
    }
  }

  /** Reset state (e.g., on disconnect or resize). */
  reset(): void {
    this.tileBatchSequencer.reset();
    this.tileDrawRuntime.reset();
    this.videoRegion = null;
    this.pendingCommands = [];
    this.stats.currentPendingCommands = 0;
  }

  /** Apply one batch atomically in protocol frame sequence order. */
  private applyBatch(batch: QueuedTileBatch): boolean {
    const { frameSeq, commands, epoch } = batch;
    if (!this.tileBatchSequencer.isCurrentEpoch(epoch)) {
      return false;
    }
    const completed = this.tileBatchCommandApplier.applyCommands({
      commands,
      frameSeq,
      epoch,
      shouldContinue: () => this.tileBatchSequencer.isCurrentEpoch(epoch),
    });
    if (!completed) {
      return false;
    }
    if (this.tileBatchSequencer.isCurrentEpoch(epoch)) {
      // Safety: ensure offset mode is restored even if TileDrawMode(true) was missed.
      this.tileDrawRuntime.restoreDefaultDrawMode();
      return true;
    }
    return false;
  }
}

export { CH_TILES };
