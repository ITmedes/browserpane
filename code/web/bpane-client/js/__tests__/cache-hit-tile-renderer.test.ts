import { describe, expect, it, vi } from 'vitest';

import { CacheHitTileRenderer } from '../render/cache-hit-tile-renderer.js';
import { TileCache } from '../tile-cache.js';

function createCanvasContext(): CanvasRenderingContext2D {
  return {
    drawImage: vi.fn(),
    putImageData: vi.fn(),
  } as unknown as CanvasRenderingContext2D;
}

function createWebGLRenderer() {
  return {
    drawTileImageBitmap: vi.fn(),
    drawTileImageData: vi.fn(),
  };
}

function createImageData(width: number, height: number): ImageData {
  return {
    width,
    height,
    data: new Uint8ClampedArray(width * height * 4),
  } as unknown as ImageData;
}

function createBitmap(width = 64, height = 64): ImageBitmap {
  return {
    width,
    height,
    close: vi.fn(),
  } as unknown as ImageBitmap;
}

describe('CacheHitTileRenderer', () => {
  it('draws cached bitmaps to the canvas renderer', () => {
    const cache = new TileCache();
    const bitmap = createBitmap();
    cache.set(1n, bitmap);
    const ctx = createCanvasContext();
    const renderer = new CacheHitTileRenderer();

    const result = renderer.draw({
      cache,
      hash: 1n,
      rect: { x: 10, y: 20, w: 64, h: 64 },
      ctx,
      glRenderer: null,
    });

    expect(result).toEqual({ kind: 'drawn' });
    expect(ctx.drawImage).toHaveBeenCalledWith(bitmap, 10, 20, 64, 64);
  });

  it('draws cached ImageData through WebGL when dimensions match', () => {
    const cache = new TileCache();
    const imageData = createImageData(64, 64);
    cache.set(2n, imageData);
    const glRenderer = createWebGLRenderer();
    const renderer = new CacheHitTileRenderer();

    const result = renderer.draw({
      cache,
      hash: 2n,
      rect: { x: 30, y: 40, w: 64, h: 64 },
      ctx: null,
      glRenderer: glRenderer as any,
    });

    expect(result).toEqual({ kind: 'drawn' });
    expect(glRenderer.drawTileImageData).toHaveBeenCalledWith(30, 40, 64, 64, imageData);
  });

  it('reports a missing cache entry as a miss', () => {
    const renderer = new CacheHitTileRenderer();

    const result = renderer.draw({
      cache: new TileCache(),
      hash: 99n,
      rect: { x: 0, y: 0, w: 64, h: 64 },
      ctx: createCanvasContext(),
      glRenderer: null,
    });

    expect(result).toEqual({ kind: 'miss', reason: 'missing' });
  });

  it('reports an ImageData size mismatch as a miss without drawing', () => {
    const cache = new TileCache();
    const imageData = createImageData(32, 64);
    cache.set(5n, imageData);
    const ctx = createCanvasContext();
    const renderer = new CacheHitTileRenderer();

    const result = renderer.draw({
      cache,
      hash: 5n,
      rect: { x: 0, y: 0, w: 64, h: 64 },
      ctx,
      glRenderer: null,
    });

    expect(result).toEqual({ kind: 'miss', reason: 'size-mismatch' });
    expect(ctx.putImageData).not.toHaveBeenCalled();
  });

  it('skips drawing when the rect is unavailable or there is no active output', () => {
    const cache = new TileCache();
    cache.set(7n, createBitmap());
    const renderer = new CacheHitTileRenderer();

    expect(renderer.draw({
      cache,
      hash: 7n,
      rect: null,
      ctx: createCanvasContext(),
      glRenderer: null,
    })).toEqual({ kind: 'skipped', reason: 'offscreen' });

    expect(renderer.draw({
      cache,
      hash: 7n,
      rect: { x: 0, y: 0, w: 64, h: 64 },
      ctx: null,
      glRenderer: null,
    })).toEqual({ kind: 'skipped', reason: 'no-output' });
  });
});
