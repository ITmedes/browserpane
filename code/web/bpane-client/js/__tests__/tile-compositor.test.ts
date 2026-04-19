import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { TileCompositor } from '../tile-compositor.js';
import { TileCache } from '../tile-cache.js';
import type { TileCommand, TileGridConfig } from '../tile-cache.js';
import { installCanvasGetContextMock } from './canvas-test-helpers.js';

// ── Mock canvas context ─────────────────────────────────────────────

function mockCtx() {
  const canvas = { width: 640, height: 640 };
  return {
    fillStyle: '',
    fillRect: vi.fn(),
    drawImage: vi.fn(),
    clearRect: vi.fn(),
    canvas,
  } as unknown as CanvasRenderingContext2D;
}

// ── Mock ImageBitmap ────────────────────────────────────────────────

function mockBitmap(id = 0): ImageBitmap {
  return { close: vi.fn(), width: 64, height: 64, _id: id } as any;
}

// ── Helpers to build wire payloads ──────────────────────────────────

function buildGridConfigPayload(config: TileGridConfig): Uint8Array {
  const buf = new Uint8Array(11);
  const view = new DataView(buf.buffer);
  buf[0] = 0x01;
  view.setUint16(1, config.tileSize, true);
  view.setUint16(3, config.cols, true);
  view.setUint16(5, config.rows, true);
  view.setUint16(7, config.screenW, true);
  view.setUint16(9, config.screenH, true);
  return buf;
}

function buildFillPayload(col: number, row: number, rgba: number): Uint8Array {
  const buf = new Uint8Array(9);
  const view = new DataView(buf.buffer);
  buf[0] = 0x03;
  view.setUint16(1, col, true);
  view.setUint16(3, row, true);
  view.setUint32(5, rgba, true);
  return buf;
}

function buildCacheHitPayload(col: number, row: number, hash: bigint): Uint8Array {
  const buf = new Uint8Array(13);
  const view = new DataView(buf.buffer);
  buf[0] = 0x02;
  view.setUint16(1, col, true);
  view.setUint16(3, row, true);
  view.setBigUint64(5, hash, true);
  return buf;
}

function buildVideoRegionPayload(x: number, y: number, w: number, h: number): Uint8Array {
  const buf = new Uint8Array(9);
  const view = new DataView(buf.buffer);
  buf[0] = 0x05;
  view.setUint16(1, x, true);
  view.setUint16(3, y, true);
  view.setUint16(5, w, true);
  view.setUint16(7, h, true);
  return buf;
}

function buildBatchEndPayload(seq: number): Uint8Array {
  const buf = new Uint8Array(5);
  const view = new DataView(buf.buffer);
  buf[0] = 0x06;
  view.setUint32(1, seq, true);
  return buf;
}

// ── Tests ───────────────────────────────────────────────────────────

describe('TileCompositor', () => {
  let compositor: TileCompositor;
  let ctx: ReturnType<typeof mockCtx>;

  beforeEach(() => {
    installCanvasGetContextMock();
    compositor = new TileCompositor();
    ctx = mockCtx();
    compositor.setContext(ctx);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('grid config', () => {
    it('stores grid config on grid-config message', () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 20, rows: 12, screenW: 1280, screenH: 768,
      }));
      const config = compositor.getGridConfig();
      expect(config).not.toBeNull();
      expect(config!.tileSize).toBe(64);
      expect(config!.cols).toBe(20);
      expect(config!.rows).toBe(12);
    });

    it('clears cache on new grid config', () => {
      const cache = compositor.getCache();
      cache.set(1n, mockBitmap());
      expect(cache.size).toBe(1);

      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));
      expect(cache.size).toBe(0);
    });

    it('clears video region on new grid config', async () => {
      compositor.processCommand({ type: 'video-region', x: 0, y: 0, w: 100, h: 100 });
      compositor.processCommand({ type: 'batch-end', frameSeq: 0 });
      await new Promise(r => setTimeout(r, 10));
      expect(compositor.getVideoRegion()).not.toBeNull();

      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));
      expect(compositor.getVideoRegion()).toBeNull();
    });
  });

  describe('fill commands', () => {
    it('draws a solid color rectangle on batch end', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 20, rows: 12, screenW: 1280, screenH: 768,
      }));

      compositor.handlePayload(buildFillPayload(0, 0, 0xFF0000FF)); // red, full alpha
      compositor.handlePayload(buildBatchEndPayload(1));

      // Wait for async flush
      await new Promise(r => setTimeout(r, 10));

      expect(ctx.fillRect).toHaveBeenCalledWith(0, 0, 64, 64);
      expect(compositor.stats.fills).toBe(1);
    });

    it('computes correct tile position', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 20, rows: 12, screenW: 1280, screenH: 768,
      }));

      compositor.handlePayload(buildFillPayload(3, 5, 0));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      expect(ctx.fillRect).toHaveBeenCalledWith(192, 320, 64, 64);
    });

    it('does not draw fill without grid config', async () => {
      compositor.handlePayload(buildFillPayload(0, 0, 0xFF));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      expect(ctx.fillRect).not.toHaveBeenCalled();
    });

    it('converts RGBA correctly', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      // RGBA in LE wire format: R=0x11, G=0x22, B=0x33, A=0xFF
      const rgba = 0xFF332211;
      compositor.handlePayload(buildFillPayload(0, 0, rgba));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      expect(ctx.fillStyle).toBe('rgba(17,34,51,1)');
    });
  });

  describe('cache-hit commands', () => {
    it('draws cached bitmap on cache hit', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      const bmp = mockBitmap();
      compositor.getCache().set(42n, bmp);

      compositor.handlePayload(buildCacheHitPayload(1, 2, 42n));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      expect(ctx.drawImage).toHaveBeenCalledWith(bmp, 64, 128, 64, 64);
      expect(compositor.stats.cacheHits).toBe(1);
    });

    it('tracks miss when hash not in cache', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      compositor.handlePayload(buildCacheHitPayload(0, 0, 999n));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      expect(ctx.drawImage).not.toHaveBeenCalled();
      expect(compositor.stats.cacheMisses).toBe(1);
    });

    it('reports cache miss with frame sequence', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));
      const onMiss = vi.fn();
      compositor.setCacheMissHandler(onMiss);

      compositor.handlePayload(buildCacheHitPayload(2, 3, 999n));
      compositor.handlePayload(buildBatchEndPayload(11));
      await new Promise(r => setTimeout(r, 10));

      expect(onMiss).toHaveBeenCalledTimes(1);
      expect(onMiss).toHaveBeenCalledWith({
        frameSeq: 11,
        col: 2,
        row: 3,
        hash: 999n,
      });
    });
  });

  describe('video-region commands', () => {
    it('updates video region', async () => {
      compositor.handlePayload(buildVideoRegionPayload(128, 64, 640, 480));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      const region = compositor.getVideoRegion();
      expect(region).toEqual({ x: 128, y: 64, w: 640, h: 480 });
    });
  });

  describe('batch processing', () => {
    it('increments batchesProcessed on batch-end', () => {
      compositor.handlePayload(buildBatchEndPayload(1));
      compositor.handlePayload(buildBatchEndPayload(2));
      expect(compositor.stats.batchesProcessed).toBe(2);
    });

    it('processes multiple commands in one batch', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      const bmp = mockBitmap();
      compositor.getCache().set(1n, bmp);

      compositor.handlePayload(buildFillPayload(0, 0, 0xFF));
      compositor.handlePayload(buildCacheHitPayload(1, 0, 1n));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      expect(compositor.stats.fills).toBe(1);
      expect(compositor.stats.cacheHits).toBe(1);
    });
  });

  describe('edge tile clamping', () => {
    it('clamps edge tile size to screen bounds', async () => {
      // Screen 100x100 with tile size 64: col=1 row=1 → tile at (64,64), clamped to 36x36
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 2, rows: 2, screenW: 100, screenH: 100,
      }));

      compositor.handlePayload(buildFillPayload(1, 1, 0xFF));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      expect(ctx.fillRect).toHaveBeenCalledWith(64, 64, 36, 36);
    });
  });

  describe('scroll-copy', () => {
    it('shifts canvas pixels on batch end', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      compositor.processCommand({ type: 'scroll-copy', dx: 0, dy: -128, regionTop: 0, regionBottom: 640, regionRight: 640 });
      expect(ctx.drawImage).toHaveBeenCalledTimes(0);

      compositor.processCommand({ type: 'batch-end', frameSeq: 1 });
      await new Promise(r => setTimeout(r, 10));

      // drawImage should be called to shift pixels
      expect(ctx.drawImage).toHaveBeenCalled();
      expect(compositor.stats.scrollCopies).toBe(1);
    });

    it('skips scroll-copy drawing when the diagnostic toggle is disabled', async () => {
      compositor.setScrollCopyEnabled(false);
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      compositor.processCommand({ type: 'scroll-copy', dx: 0, dy: -128, regionTop: 0, regionBottom: 640, regionRight: 640 });
      compositor.processCommand({ type: 'batch-end', frameSeq: 1 });
      await new Promise(r => setTimeout(r, 10));

      expect(ctx.drawImage).not.toHaveBeenCalled();
      expect(compositor.stats.scrollCopies).toBe(0);
    });

    it('clips and redraws a viewport-only scroll at the shifted Y position', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      const scratchCtx = {
        clearRect: vi.fn(),
        drawImage: vi.fn(),
      };
      const scratchCanvas = {
        width: 0,
        height: 0,
        getContext: vi.fn(() => scratchCtx),
      };
      const originalCreateElement = document.createElement.bind(document);
      const createElementSpy = vi.spyOn(document, 'createElement').mockImplementation(((tagName: string) => {
        if (tagName === 'canvas') return scratchCanvas as unknown as HTMLCanvasElement;
        return originalCreateElement(tagName);
      }) as typeof document.createElement);

      compositor.processCommand({
        type: 'scroll-copy',
        dx: 0,
        dy: -64,
        regionTop: 64,
        regionBottom: 320,
        regionRight: 400,
      });
      compositor.processCommand({ type: 'batch-end', frameSeq: 1 });
      await new Promise(r => setTimeout(r, 10));

      expect(ctx.clearRect).not.toHaveBeenCalled();
      expect(ctx.drawImage).toHaveBeenCalledWith(
        scratchCanvas,
        0, 0, 400, 192,
        0, 128, 400, 192,
      );

      createElementSpy.mockRestore();
    });

    it('defers scroll-copy until batch-end', () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      compositor.processCommand({ type: 'scroll-copy', dx: 64, dy: 0, regionTop: 0, regionBottom: 640, regionRight: 640 });
      expect(ctx.drawImage).toHaveBeenCalledTimes(0);
    });
  });

  describe('reset', () => {
    it('clears cache, grid config, and video region', () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));
      compositor.getCache().set(1n, mockBitmap());
      compositor.processCommand({ type: 'video-region', x: 0, y: 0, w: 100, h: 100 });
      compositor.processCommand({ type: 'batch-end', frameSeq: 0 });

      compositor.reset();

      expect(compositor.getGridConfig()).toBeNull();
      expect(compositor.getVideoRegion()).toBeNull();
      expect(compositor.getCache().size).toBe(0);
    });
  });

  describe('handlePayload', () => {
    it('returns null for malformed payload', () => {
      expect(compositor.handlePayload(new Uint8Array(0))).toBeNull();
      expect(compositor.handlePayload(new Uint8Array([0xFF]))).toBeNull();
    });

    it('returns parsed command for valid payload', () => {
      const cmd = compositor.handlePayload(buildBatchEndPayload(42));
      expect(cmd).not.toBeNull();
      expect(cmd!.type).toBe('batch-end');
    });
  });

  describe('grid-offset', () => {
    function buildGridOffsetPayload(offsetX: number, offsetY: number): Uint8Array {
      const buf = new Uint8Array(5);
      const view = new DataView(buf.buffer);
      buf[0] = 0x08;
      view.setInt16(1, offsetX, true);
      view.setInt16(3, offsetY, true);
      return buf;
    }

    it('shifts tile position by grid offset', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      // Set grid offset of 37 pixels
      compositor.handlePayload(buildGridOffsetPayload(0, 37));

      // Fill tile at (1, 2) — should be drawn at (64, 128-37) = (64, 91)
      compositor.handlePayload(buildFillPayload(1, 2, 0xFF));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      expect(ctx.fillRect).toHaveBeenCalledWith(64, 91, 64, 64);
    });

    it('clamps top partial tile to screen top', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      compositor.handlePayload(buildGridOffsetPayload(0, 37));

      // Tile (0, 0): rawY = 0 - 37 = -37, clamped to y=0, h=27
      compositor.handlePayload(buildFillPayload(0, 0, 0xFF));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      expect(ctx.fillRect).toHaveBeenCalledWith(0, 0, 64, 27);
    });

    it('resets offset on new grid config', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      compositor.handlePayload(buildGridOffsetPayload(0, 37));

      // New grid config should reset offset
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      // Tile (1, 2) should be at normal position (64, 128)
      compositor.handlePayload(buildFillPayload(1, 2, 0xFF));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      expect(ctx.fillRect).toHaveBeenCalledWith(64, 128, 64, 64);
    });

    it('parses grid-offset command from payload', () => {
      const cmd = compositor.handlePayload(buildGridOffsetPayload(10, 37));
      expect(cmd).not.toBeNull();
      expect(cmd!.type).toBe('grid-offset');
    });
  });

  // content-space cache tests removed — contentHashes feature was removed in Phase 1.4
  // (dead code: written but never read, no production effect).

  describe('frame sequencing', () => {
    it('drops out-of-order older batches by frameSeq', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      // Apply newer frame first.
      compositor.handlePayload(buildFillPayload(0, 0, 0xFF));
      compositor.handlePayload(buildBatchEndPayload(2));

      // Stale older frame arrives late — must be ignored.
      compositor.handlePayload(buildFillPayload(1, 0, 0xFF));
      compositor.handlePayload(buildBatchEndPayload(1));

      await new Promise(r => setTimeout(r, 10));

      expect(ctx.fillRect).toHaveBeenCalledTimes(1);
      expect(ctx.fillRect).toHaveBeenCalledWith(0, 0, 64, 64);
    });
  });

  describe('batch staleness', () => {
    it('stale PNG decode does not overwrite newer batch', async () => {
      compositor.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));

      // Pre-populate cache so cache-hit works
      const bmp = mockBitmap(99);
      compositor.getCache().set(99n, bmp);

      // Batch 1: cache-hit at (0,0) — draws immediately
      compositor.handlePayload(buildCacheHitPayload(0, 0, 99n));
      compositor.handlePayload(buildBatchEndPayload(1));

      // Batch 2 arrives immediately — newer batch
      compositor.handlePayload(buildCacheHitPayload(0, 0, 99n));
      compositor.handlePayload(buildBatchEndPayload(2));

      await new Promise(r => setTimeout(r, 10));

      // Both batches processed without errors
      expect(compositor.stats.batchesProcessed).toBe(2);
    });
  });

  describe('no context', () => {
    it('does not crash when no context is set', async () => {
      const noCtx = new TileCompositor();
      noCtx.handlePayload(buildGridConfigPayload({
        tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640,
      }));
      noCtx.handlePayload(buildFillPayload(0, 0, 0xFF));
      noCtx.handlePayload(buildBatchEndPayload(1));
      await new Promise(r => setTimeout(r, 10));
      // Should not throw
    });
  });
});
