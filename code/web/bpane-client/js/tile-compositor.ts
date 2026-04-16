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
import { QoiTileRenderer } from './render/qoi-tile-renderer.js';
import { resolveTileRect } from './render/tile-rect-resolver.js';
import { ZstdTileRenderer } from './render/zstd-tile-renderer.js';
import type { WebGLTileRenderer } from './webgl-compositor.js';

export interface CompositorStats {
  fills: number;
  cacheHits: number;
  cacheMisses: number;
  qoiDecodes: number;
  qoiRedundant: number;
  qoiRedundantBytes: number;
  zstdDecodes: number;
  zstdRedundant: number;
  zstdRedundantBytes: number;
  batchesProcessed: number;
  scrollCopies: number;
}

export interface CacheMissEvent {
  frameSeq: number;
  col: number;
  row: number;
  hash: bigint;
}

export class TileCompositor {
  private cache: TileCache;
  private gridConfig: TileGridConfig | null = null;
  private videoRegion: { x: number; y: number; w: number; h: number } | null = null;
  private pendingCommands: TileCommand[] = [];
  private ctx: CanvasRenderingContext2D | null = null;
  private glRenderer: WebGLTileRenderer | null = null;
  private gridOffsetX = 0;
  private gridOffsetY = 0;
  private flushChain: Promise<void> = Promise.resolve();
  private lastAppliedFrameSeq: number | null = null;
  private epoch = 0;
  private activeBatchFrameSeq: number | null = null;
  private onCacheMiss: ((event: CacheMissEvent) => void) | null = null;
  private readonly cacheHitTileRenderer: CacheHitTileRenderer;
  private readonly qoiTileRenderer: QoiTileRenderer;
  private readonly zstdTileRenderer: ZstdTileRenderer;
  private readonly canvasScrollCopyRenderer: CanvasScrollCopyRenderer;

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
  };

  constructor(
    cache?: TileCache,
    cacheHitTileRenderer: CacheHitTileRenderer = new CacheHitTileRenderer(),
    qoiTileRenderer: QoiTileRenderer = new QoiTileRenderer(),
    zstdTileRenderer: ZstdTileRenderer = new ZstdTileRenderer(),
    canvasScrollCopyRenderer: CanvasScrollCopyRenderer = new CanvasScrollCopyRenderer(),
  ) {
    this.cache = cache ?? new TileCache();
    this.cacheHitTileRenderer = cacheHitTileRenderer;
    this.qoiTileRenderer = qoiTileRenderer;
    this.zstdTileRenderer = zstdTileRenderer;
    this.canvasScrollCopyRenderer = canvasScrollCopyRenderer;
  }

  /** Bind to a canvas rendering context for drawing (Canvas2D fallback). */
  setContext(ctx: CanvasRenderingContext2D): void {
    this.ctx = ctx;
  }

  /** Bind a WebGL2 renderer for GPU-accelerated drawing. When set, draw calls
   *  are routed to WebGL instead of Canvas2D. Pass null to revert to Canvas2D. */
  setWebGLRenderer(renderer: WebGLTileRenderer | null): void {
    this.glRenderer = renderer;
  }

  /** Whether a WebGL renderer is currently active. */
  hasWebGL(): boolean {
    return this.glRenderer !== null;
  }

  /** Current tile grid configuration, or null if not yet received. */
  getGridConfig(): TileGridConfig | null {
    return this.gridConfig;
  }

  /** Current video region bounding box, or null. */
  getVideoRegion(): { x: number; y: number; w: number; h: number } | null {
    return this.videoRegion;
  }

  /** Access the underlying tile cache. */
  getCache(): TileCache {
    return this.cache;
  }

  /** Register a callback for cache-miss telemetry back to the host. */
  setCacheMissHandler(handler: ((event: CacheMissEvent) => void) | null): void {
    this.onCacheMiss = handler;
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
        this.epoch++;
        this.pendingCommands = [];
        this.lastAppliedFrameSeq = null;
        this.applyGridConfig(cmd.config);
        break;

      case 'batch-end':
        // Frame-sequenced, serialized batch processing.
        this.enqueueBatch(cmd.frameSeq >>> 0, this.pendingCommands, this.epoch);
        this.pendingCommands = [];
        this.stats.batchesProcessed++;
        break;

      case 'scroll-stats':
        // Telemetry-only sideband command (handled by session stats layer).
        break;

      default:
        this.pendingCommands.push(cmd);
        break;
    }
  }

  /** Reset state (e.g., on disconnect or resize). */
  reset(): void {
    this.epoch++;
    this.cache.clear();
    this.gridConfig = null;
    this.videoRegion = null;
    this.pendingCommands = [];
    this.gridOffsetX = 0;
    this.gridOffsetY = 0;
    this.flushChain = Promise.resolve();
    this.lastAppliedFrameSeq = null;
    this.activeBatchFrameSeq = null;
    this.canvasScrollCopyRenderer.reset();
    this.applyOffsetMode = true;
  }

  private applyGridConfig(config: TileGridConfig): void {
    this.gridConfig = config;
    this.cache.clear();
    this.videoRegion = null;
    this.gridOffsetX = 0;
    this.gridOffsetY = 0;
    this.applyOffsetMode = true;
  }

  private isNewerFrameSeq(seq: number): boolean {
    if (this.lastAppliedFrameSeq === null) return true;
    const diff = (seq - this.lastAppliedFrameSeq) >>> 0;
    return diff !== 0 && diff < 0x80000000;
  }

  private enqueueBatch(frameSeq: number, commands: TileCommand[], epoch: number): void {
    const batch = [...commands];
    this.flushChain = this.flushChain.then(() => this.applyBatch(frameSeq, batch, epoch));
  }

  /** Whether tile draws should apply gridOffset (true for content tiles, false for static header/scrollbar tiles). */
  private applyOffsetMode = true;

  private applyScrollCopy(dx: number, dy: number, regionTop: number, regionBottom: number, regionRight: number): void {
    if (!this.gridConfig) return;

    // WebGL path: delegate scroll to the GPU renderer
    if (this.glRenderer) {
      this.glRenderer.scrollCopy(
        dx, dy, regionTop, regionBottom, regionRight,
        this.gridConfig.screenW, this.gridConfig.screenH,
      );
      this.stats.scrollCopies++;
      return;
    }

    if (!this.ctx) return;
    if (!this.canvasScrollCopyRenderer.apply({
      ctx: this.ctx,
      dx,
      dy,
      regionTop,
      regionBottom,
      regionRight,
      screenW: this.gridConfig.screenW,
      screenH: this.gridConfig.screenH,
    })) {
      return;
    }
    this.stats.scrollCopies++;
  }

  /** Apply one batch atomically in protocol frame sequence order. */
  private async applyBatch(frameSeq: number, commands: TileCommand[], epoch: number): Promise<void> {
    if (epoch !== this.epoch) return;
    if (!this.isNewerFrameSeq(frameSeq)) return;
    this.activeBatchFrameSeq = frameSeq;
    for (const cmd of commands) {
      if (epoch !== this.epoch) return;

      switch (cmd.type) {
        case 'scroll-copy':
          this.applyScrollCopy(cmd.dx, cmd.dy, cmd.regionTop, cmd.regionBottom, cmd.regionRight);
          break;

        case 'grid-offset':
          this.gridOffsetX = cmd.offsetX;
          this.gridOffsetY = cmd.offsetY;
          break;

        case 'tile-draw-mode':
          this.applyOffsetMode = cmd.applyOffset;
          break;

        case 'fill':
          this.drawFill(cmd.col, cmd.row, cmd.rgba);
          break;

        case 'cache-hit':
          this.drawCacheHit(cmd.col, cmd.row, cmd.hash, frameSeq);
          break;

        case 'qoi':
          this.drawQoi(cmd.col, cmd.row, cmd.hash, cmd.data, epoch);
          break;

        case 'zstd':
          this.drawZstd(cmd.col, cmd.row, cmd.hash, cmd.data, epoch);
          break;

        case 'video-region':
          // Zero-size region means video ended — clear it.
          this.videoRegion = (cmd.w > 0 && cmd.h > 0)
            ? { x: cmd.x, y: cmd.y, w: cmd.w, h: cmd.h }
            : null;
          break;

        case 'grid-config':
        case 'batch-end':
        case 'scroll-stats':
          // Not expected in queued per-frame command list.
          break;
      }
    }
    if (epoch === this.epoch) {
      this.lastAppliedFrameSeq = frameSeq;
      this.activeBatchFrameSeq = null;
      // Safety: ensure offset mode is restored even if TileDrawMode(true) was missed.
      this.applyOffsetMode = true;
    }
  }

  private tileRect(col: number, row: number): { x: number; y: number; w: number; h: number } | null {
    return resolveTileRect({
      gridConfig: this.gridConfig,
      col,
      row,
      gridOffsetX: this.gridOffsetX,
      gridOffsetY: this.gridOffsetY,
      applyOffset: this.applyOffsetMode,
    });
  }

  private drawFill(col: number, row: number, rgba: number): void {
    if (!this.ctx && !this.glRenderer) return;
    const rect = this.tileRect(col, row);
    if (!rect) return;

    const r = (rgba >>> 0) & 0xFF;
    const g = (rgba >>> 8) & 0xFF;
    const b = (rgba >>> 16) & 0xFF;
    const a = ((rgba >>> 24) & 0xFF) / 255;

    if (this.glRenderer) {
      this.glRenderer.drawFill(rect.x, rect.y, rect.w, rect.h, r, g, b, a);
    } else if (this.ctx) {
      this.ctx.fillStyle = `rgba(${r},${g},${b},${a})`;
      this.ctx.fillRect(rect.x, rect.y, rect.w, rect.h);
    }
    this.stats.fills++;
  }

  private drawCacheHit(col: number, row: number, hash: bigint, frameSeq: number): void {
    const result = this.cacheHitTileRenderer.draw({
      cache: this.cache,
      hash,
      rect: this.tileRect(col, row),
      ctx: this.ctx,
      glRenderer: this.glRenderer,
    });

    if (result.kind === 'drawn') {
      this.stats.cacheHits++;
      return;
    }

    if (result.kind === 'miss') {
      this.stats.cacheMisses++;
      this.onCacheMiss?.({ frameSeq, col, row, hash });
    }
  }

  private drawQoi(col: number, row: number, hash: bigint, data: Uint8Array, epoch: number): void {
    const result = this.qoiTileRenderer.draw({
      cache: this.cache,
      hash,
      data,
      rect: this.tileRect(col, row),
      ctx: this.ctx,
      glRenderer: this.glRenderer,
      shouldDraw: () => epoch === this.epoch,
    });

    if (result.kind === 'drawn' || result.kind === 'cached') {
      if (result.redundant) {
        this.stats.qoiRedundant++;
        this.stats.qoiRedundantBytes += result.decodedBytes;
      }
      if (result.kind === 'cached') return;
      this.stats.qoiDecodes++;
      return;
    }

    if (result.kind === 'miss') {
      this.stats.cacheMisses++;
    }
  }
  private drawZstd(col: number, row: number, hash: bigint, data: Uint8Array, epoch: number): void {
    const result = this.zstdTileRenderer.draw({
      cache: this.cache,
      hash,
      data,
      rect: this.tileRect(col, row),
      ctx: this.ctx,
      glRenderer: this.glRenderer,
      shouldDraw: () => epoch === this.epoch,
    });

    if (result.kind === 'drawn' || result.kind === 'cached') {
      if (result.redundant) {
        this.stats.zstdRedundant++;
        this.stats.zstdRedundantBytes += result.encodedBytes;
      }
      if (result.kind === 'cached') return;
      this.stats.zstdDecodes++;
      return;
    }

    if (result.kind === 'miss') {
      this.stats.cacheMisses++;
    }
  }
}

export { CH_TILES };
