import { describe, expect, it, vi } from 'vitest';

import { TileCache } from '../tile-cache.js';
import {
  TileDrawRuntime,
  type TileCompositorRenderStats,
} from '../render/tile-draw-runtime.js';

function createStats(): TileCompositorRenderStats {
  return {
    fills: 0,
    cacheHits: 0,
    cacheMisses: 0,
    qoiDecodes: 0,
    qoiRedundant: 0,
    qoiRedundantBytes: 0,
    zstdDecodes: 0,
    zstdRedundant: 0,
    zstdRedundantBytes: 0,
    scrollCopies: 0,
  };
}

describe('TileDrawRuntime', () => {
  it('resolves offset-aware fill rects and honors draw mode changes', () => {
    const fillTileRenderer = {
      draw: vi.fn(() => ({ kind: 'drawn' as const })),
    };
    const runtime = new TileDrawRuntime({
      cache: new TileCache(),
      stats: createStats(),
      fillTileRenderer: fillTileRenderer as any,
    });

    runtime.applyGridConfig({
      tileSize: 64,
      cols: 10,
      rows: 10,
      screenW: 640,
      screenH: 640,
    });
    runtime.setGridOffset(0, 37);

    runtime.drawFill(1, 2, 0xff);
    expect(fillTileRenderer.draw).toHaveBeenLastCalledWith(expect.objectContaining({
      rect: { x: 64, y: 91, w: 64, h: 64 },
      rgba: 0xff,
    }));
    expect(runtime.getStats().fills).toBe(1);

    runtime.setApplyOffsetMode(false);
    runtime.drawFill(1, 2, 0xff);
    expect(fillTileRenderer.draw).toHaveBeenLastCalledWith(expect.objectContaining({
      rect: { x: 64, y: 128, w: 64, h: 64 },
      rgba: 0xff,
    }));
    expect(runtime.getStats().fills).toBe(2);
  });

  it('tracks cache misses and reports the exact miss event', () => {
    const cacheHitTileRenderer = {
      draw: vi.fn(() => ({ kind: 'miss' as const, reason: 'missing' as const })),
    };
    const onCacheMiss = vi.fn();
    const runtime = new TileDrawRuntime({
      cache: new TileCache(),
      stats: createStats(),
      cacheHitTileRenderer: cacheHitTileRenderer as any,
    });
    runtime.setCacheMissHandler(onCacheMiss);

    runtime.applyGridConfig({
      tileSize: 64,
      cols: 10,
      rows: 10,
      screenW: 640,
      screenH: 640,
    });
    runtime.drawCacheHit(3, 4, 99n, 12);

    expect(runtime.getStats().cacheMisses).toBe(1);
    expect(onCacheMiss).toHaveBeenCalledWith({
      frameSeq: 12,
      col: 3,
      row: 4,
      hash: 99n,
    });
  });

  it('counts redundant cached qoi completions without incrementing decode totals', () => {
    const qoiTileRenderer = {
      draw: vi.fn(() => ({ kind: 'cached' as const, redundant: true, decodedBytes: 42 })),
    };
    const runtime = new TileDrawRuntime({
      cache: new TileCache(),
      stats: createStats(),
      qoiTileRenderer: qoiTileRenderer as any,
    });

    runtime.applyGridConfig({
      tileSize: 64,
      cols: 10,
      rows: 10,
      screenW: 640,
      screenH: 640,
    });
    runtime.drawQoi(0, 0, 1n, new Uint8Array([1, 2, 3]), () => false);

    expect(runtime.getStats().qoiRedundant).toBe(1);
    expect(runtime.getStats().qoiRedundantBytes).toBe(42);
    expect(runtime.getStats().qoiDecodes).toBe(0);
    expect(runtime.getStats().cacheMisses).toBe(0);
  });

  it('increments zstd decode totals for drawn tiles and cache misses for failures', () => {
    const zstdTileRenderer = {
      draw: vi
        .fn()
        .mockReturnValueOnce({ kind: 'drawn' as const, redundant: false, encodedBytes: 7 })
        .mockReturnValueOnce({ kind: 'miss' as const, reason: 'decode-failed' as const }),
    };
    const runtime = new TileDrawRuntime({
      cache: new TileCache(),
      stats: createStats(),
      zstdTileRenderer: zstdTileRenderer as any,
    });

    runtime.applyGridConfig({
      tileSize: 64,
      cols: 10,
      rows: 10,
      screenW: 640,
      screenH: 640,
    });
    runtime.drawZstd(0, 0, 2n, new Uint8Array([4, 5, 6]), () => true);
    runtime.drawZstd(0, 0, 3n, new Uint8Array([7, 8, 9]), () => true);

    expect(runtime.getStats().zstdDecodes).toBe(1);
    expect(runtime.getStats().cacheMisses).toBe(1);
  });

  it('increments scroll copy stats only when a copy is actually applied', () => {
    const canvasScrollCopyRenderer = {
      apply: vi
        .fn()
        .mockReturnValueOnce(false)
        .mockReturnValueOnce(true),
      reset: vi.fn(),
    };
    const runtime = new TileDrawRuntime({
      cache: new TileCache(),
      stats: createStats(),
      canvasScrollCopyRenderer: canvasScrollCopyRenderer as any,
    });
    runtime.setContext({ canvas: { width: 640, height: 640 } } as CanvasRenderingContext2D);
    runtime.applyGridConfig({
      tileSize: 64,
      cols: 10,
      rows: 10,
      screenW: 640,
      screenH: 640,
    });

    runtime.applyScrollCopy(0, -64, 0, 640, 640);
    runtime.applyScrollCopy(0, -64, 0, 640, 640);

    expect(runtime.getStats().scrollCopies).toBe(1);
    expect(canvasScrollCopyRenderer.apply).toHaveBeenCalledTimes(2);
  });
});
