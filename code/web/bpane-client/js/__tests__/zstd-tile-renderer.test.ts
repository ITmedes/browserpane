import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ZstdTileRenderer } from '../render/zstd-tile-renderer.js';
import { TileCache } from '../tile-cache.js';
import { installCanvasGetContextMock } from './canvas-test-helpers.js';

function createCanvasContext(): CanvasRenderingContext2D {
  return {
    putImageData: vi.fn(),
  } as unknown as CanvasRenderingContext2D;
}

function createWebGLRenderer() {
  return {
    drawTileImageData: vi.fn(),
  };
}

function createCompressedRgbaPixel(): Uint8Array {
  return new Uint8Array([
    0x28, 0xb5, 0x2f, 0xfd, 0x24, 0x04, 0x21, 0x00,
    0x00, 0x11, 0x22, 0x33, 0x44, 0x98, 0xcb, 0x4d, 0x84,
  ]);
}

describe('ZstdTileRenderer', () => {
  beforeEach(() => {
    installCanvasGetContextMock();
    (globalThis as any).ImageData = class MockImageData {
      data: Uint8ClampedArray;
      width: number;
      height: number;

      constructor(data: Uint8ClampedArray, width: number, height: number) {
        this.data = data;
        this.width = width;
        this.height = height;
      }
    };
  });

  it('decompresses, caches, and draws a zstd tile to canvas output', () => {
    const cache = new TileCache();
    const ctx = createCanvasContext();
    const renderer = new ZstdTileRenderer();

    const result = renderer.draw({
      cache,
      hash: 1n,
      data: createCompressedRgbaPixel(),
      rect: { x: 5, y: 6, w: 1, h: 1 },
      ctx,
      glRenderer: null,
    });

    expect(result).toEqual({
      kind: 'drawn',
      redundant: false,
      encodedBytes: createCompressedRgbaPixel().byteLength,
    });
    expect(cache.get(1n)).toBeInstanceOf(ImageData);
    expect(ctx.putImageData).toHaveBeenCalledWith(expect.any(ImageData), 5, 6);
  });

  it('draws through WebGL when that output is active', () => {
    const cache = new TileCache();
    const glRenderer = createWebGLRenderer();
    const renderer = new ZstdTileRenderer();

    const result = renderer.draw({
      cache,
      hash: 2n,
      data: createCompressedRgbaPixel(),
      rect: { x: 10, y: 20, w: 1, h: 1 },
      ctx: null,
      glRenderer: glRenderer as any,
    });

    expect(result.kind).toBe('drawn');
    expect(glRenderer.drawTileImageData).toHaveBeenCalledWith(10, 20, 1, 1, expect.any(ImageData));
  });

  it('marks redundant draws when the tile hash already exists in cache', () => {
    const cache = new TileCache();
    cache.set(3n, { width: 1, height: 1, data: new Uint8ClampedArray(4) } as unknown as ImageData);
    const renderer = new ZstdTileRenderer();

    const result = renderer.draw({
      cache,
      hash: 3n,
      data: createCompressedRgbaPixel(),
      rect: { x: 0, y: 0, w: 1, h: 1 },
      ctx: createCanvasContext(),
      glRenderer: null,
    });

    expect(result).toEqual({
      kind: 'drawn',
      redundant: true,
      encodedBytes: createCompressedRgbaPixel().byteLength,
    });
  });

  it('caches but does not draw when shouldDraw rejects a stale completion', () => {
    const cache = new TileCache();
    const ctx = createCanvasContext();
    const renderer = new ZstdTileRenderer();

    const result = renderer.draw({
      cache,
      hash: 4n,
      data: createCompressedRgbaPixel(),
      rect: { x: 0, y: 0, w: 1, h: 1 },
      ctx,
      glRenderer: null,
      shouldDraw: () => false,
    });

    expect(result).toEqual({
      kind: 'cached',
      redundant: false,
      encodedBytes: createCompressedRgbaPixel().byteLength,
    });
    expect(cache.get(4n)).toBeInstanceOf(ImageData);
    expect(ctx.putImageData).not.toHaveBeenCalled();
  });

  it('reports decode failure for malformed payloads', () => {
    const renderer = new ZstdTileRenderer();

    const result = renderer.draw({
      cache: new TileCache(),
      hash: 5n,
      data: new Uint8Array([1, 2, 3]),
      rect: { x: 0, y: 0, w: 1, h: 1 },
      ctx: createCanvasContext(),
      glRenderer: null,
    });

    expect(result).toEqual({ kind: 'miss', reason: 'decode-failed' });
  });

  it('reports a size mismatch when decompressed bytes do not match the target rect', () => {
    const renderer = new ZstdTileRenderer();

    const result = renderer.draw({
      cache: new TileCache(),
      hash: 6n,
      data: createCompressedRgbaPixel(),
      rect: { x: 0, y: 0, w: 2, h: 1 },
      ctx: createCanvasContext(),
      glRenderer: null,
    });

    expect(result).toEqual({ kind: 'miss', reason: 'size-mismatch' });
  });

  it('skips drawing when the tile is offscreen or there is no active output', () => {
    const renderer = new ZstdTileRenderer();

    expect(renderer.draw({
      cache: new TileCache(),
      hash: 7n,
      data: createCompressedRgbaPixel(),
      rect: null,
      ctx: createCanvasContext(),
      glRenderer: null,
    })).toEqual({ kind: 'skipped', reason: 'offscreen' });

    expect(renderer.draw({
      cache: new TileCache(),
      hash: 8n,
      data: createCompressedRgbaPixel(),
      rect: { x: 0, y: 0, w: 1, h: 1 },
      ctx: null,
      glRenderer: null,
    })).toEqual({ kind: 'skipped', reason: 'no-output' });
  });
});
