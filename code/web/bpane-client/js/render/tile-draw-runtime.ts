import type { CacheHitTileDrawResult } from './cache-hit-tile-renderer.js';
import { CacheHitTileRenderer } from './cache-hit-tile-renderer.js';
import { CanvasScrollCopyRenderer } from './canvas-scroll-copy-renderer.js';
import type { FillTileDrawResult } from './fill-tile-renderer.js';
import { FillTileRenderer } from './fill-tile-renderer.js';
import type { QoiTileDrawResult } from './qoi-tile-renderer.js';
import { QoiTileRenderer } from './qoi-tile-renderer.js';
import { resolveTileRect } from './tile-rect-resolver.js';
import type { ZstdTileDrawResult } from './zstd-tile-renderer.js';
import { ZstdTileRenderer } from './zstd-tile-renderer.js';
import type { TileCache, TileGridConfig } from '../tile-cache.js';
import { TileCache as DefaultTileCache } from '../tile-cache.js';
import type { WebGLTileRenderer } from '../webgl-compositor.js';

export interface TileCompositorRenderStats {
  fills: number;
  cacheHits: number;
  cacheMisses: number;
  qoiDecodes: number;
  qoiRedundant: number;
  qoiRedundantBytes: number;
  zstdDecodes: number;
  zstdRedundant: number;
  zstdRedundantBytes: number;
  scrollCopies: number;
}

export interface CacheMissEvent {
  frameSeq: number;
  col: number;
  row: number;
  hash: bigint;
}

type FillTileRendererLike = {
  draw(args: Parameters<FillTileRenderer['draw']>[0]): FillTileDrawResult;
};

type CacheHitTileRendererLike = {
  draw(args: Parameters<CacheHitTileRenderer['draw']>[0]): CacheHitTileDrawResult;
};

type QoiTileRendererLike = {
  draw(args: Parameters<QoiTileRenderer['draw']>[0]): QoiTileDrawResult;
};

type ZstdTileRendererLike = {
  draw(args: Parameters<ZstdTileRenderer['draw']>[0]): ZstdTileDrawResult;
};

type CanvasScrollCopyRendererLike = {
  apply(args: Parameters<CanvasScrollCopyRenderer['apply']>[0]): boolean;
  reset(): void;
};

export class TileDrawRuntime {
  private readonly cache: TileCache;
  private readonly stats: TileCompositorRenderStats;
  private gridConfig: TileGridConfig | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private glRenderer: WebGLTileRenderer | null = null;
  private gridOffsetX = 0;
  private gridOffsetY = 0;
  private applyOffsetMode = true;
  private onCacheMiss: ((event: CacheMissEvent) => void) | null = null;
  private scrollCopyEnabled = true;
  private readonly fillTileRenderer: FillTileRendererLike;
  private readonly cacheHitTileRenderer: CacheHitTileRendererLike;
  private readonly qoiTileRenderer: QoiTileRendererLike;
  private readonly zstdTileRenderer: ZstdTileRendererLike;
  private readonly canvasScrollCopyRenderer: CanvasScrollCopyRendererLike;

  constructor(args: {
    cache?: TileCache;
    stats: TileCompositorRenderStats;
    fillTileRenderer?: FillTileRendererLike;
    cacheHitTileRenderer?: CacheHitTileRendererLike;
    qoiTileRenderer?: QoiTileRendererLike;
    zstdTileRenderer?: ZstdTileRendererLike;
    canvasScrollCopyRenderer?: CanvasScrollCopyRendererLike;
  }) {
    this.cache = args.cache ?? new DefaultTileCache();
    this.stats = args.stats;
    this.fillTileRenderer = args.fillTileRenderer ?? new FillTileRenderer();
    this.cacheHitTileRenderer = args.cacheHitTileRenderer ?? new CacheHitTileRenderer();
    this.qoiTileRenderer = args.qoiTileRenderer ?? new QoiTileRenderer();
    this.zstdTileRenderer = args.zstdTileRenderer ?? new ZstdTileRenderer();
    this.canvasScrollCopyRenderer = args.canvasScrollCopyRenderer ?? new CanvasScrollCopyRenderer();
  }

  getStats(): TileCompositorRenderStats {
    return this.stats;
  }

  setContext(ctx: CanvasRenderingContext2D | null): void {
    this.ctx = ctx;
  }

  setWebGLRenderer(renderer: WebGLTileRenderer | null): void {
    this.glRenderer = renderer;
  }

  hasWebGL(): boolean {
    return this.glRenderer !== null;
  }

  getGridConfig(): TileGridConfig | null {
    return this.gridConfig;
  }

  getCache(): TileCache {
    return this.cache;
  }

  setCacheMissHandler(handler: ((event: CacheMissEvent) => void) | null): void {
    this.onCacheMiss = handler;
  }

  setScrollCopyEnabled(enabled: boolean): void {
    this.scrollCopyEnabled = enabled;
  }

  applyGridConfig(config: TileGridConfig): void {
    this.gridConfig = config;
    this.cache.clear();
    this.gridOffsetX = 0;
    this.gridOffsetY = 0;
    this.applyOffsetMode = true;
  }

  reset(): void {
    this.cache.clear();
    this.gridConfig = null;
    this.gridOffsetX = 0;
    this.gridOffsetY = 0;
    this.applyOffsetMode = true;
    this.canvasScrollCopyRenderer.reset();
  }

  setGridOffset(offsetX: number, offsetY: number): void {
    this.gridOffsetX = offsetX;
    this.gridOffsetY = offsetY;
  }

  setApplyOffsetMode(applyOffset: boolean): void {
    this.applyOffsetMode = applyOffset;
  }

  restoreDefaultDrawMode(): void {
    this.applyOffsetMode = true;
  }

  applyScrollCopy(dx: number, dy: number, regionTop: number, regionBottom: number, regionRight: number): void {
    if (!this.scrollCopyEnabled) {
      return;
    }
    if (!this.gridConfig) {
      return;
    }

    if (this.glRenderer) {
      this.glRenderer.scrollCopy(
        dx,
        dy,
        regionTop,
        regionBottom,
        regionRight,
        this.gridConfig.screenW,
        this.gridConfig.screenH,
      );
      this.stats.scrollCopies++;
      return;
    }

    if (!this.ctx) {
      return;
    }
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

  drawFill(col: number, row: number, rgba: number): void {
    const result = this.fillTileRenderer.draw({
      rect: this.tileRect(col, row),
      rgba,
      ctx: this.ctx,
      glRenderer: this.glRenderer,
    });

    if (result.kind === 'drawn') {
      this.stats.fills++;
    }
  }

  drawCacheHit(col: number, row: number, hash: bigint, frameSeq: number): void {
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

  drawQoi(col: number, row: number, hash: bigint, data: Uint8Array, shouldDraw: () => boolean): void {
    const result = this.qoiTileRenderer.draw({
      cache: this.cache,
      hash,
      data,
      rect: this.tileRect(col, row),
      ctx: this.ctx,
      glRenderer: this.glRenderer,
      shouldDraw,
    });

    if (result.kind === 'drawn' || result.kind === 'cached') {
      if (result.redundant) {
        this.stats.qoiRedundant++;
        this.stats.qoiRedundantBytes += result.decodedBytes;
      }
      if (result.kind === 'cached') {
        return;
      }
      this.stats.qoiDecodes++;
      return;
    }

    if (result.kind === 'miss') {
      this.stats.cacheMisses++;
    }
  }

  drawZstd(col: number, row: number, hash: bigint, data: Uint8Array, shouldDraw: () => boolean): void {
    const result = this.zstdTileRenderer.draw({
      cache: this.cache,
      hash,
      data,
      rect: this.tileRect(col, row),
      ctx: this.ctx,
      glRenderer: this.glRenderer,
      shouldDraw,
    });

    if (result.kind === 'drawn' || result.kind === 'cached') {
      if (result.redundant) {
        this.stats.zstdRedundant++;
        this.stats.zstdRedundantBytes += result.encodedBytes;
      }
      if (result.kind === 'cached') {
        return;
      }
      this.stats.zstdDecodes++;
      return;
    }

    if (result.kind === 'miss') {
      this.stats.cacheMisses++;
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
}
