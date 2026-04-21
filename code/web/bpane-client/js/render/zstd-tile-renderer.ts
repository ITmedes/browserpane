import { decompress } from 'fzstd';

import type { TileCache } from '../tile-cache.js';
import type { WebGLTileRenderer } from '../webgl-compositor.js';

export type ZstdTileDrawResult =
  | { kind: 'drawn'; redundant: boolean; encodedBytes: number }
  | { kind: 'cached'; redundant: boolean; encodedBytes: number }
  | { kind: 'miss'; reason: 'decode-failed' | 'size-mismatch' }
  | { kind: 'skipped'; reason: 'offscreen' | 'no-output' };

export class ZstdTileRenderer {
  private readonly decompressFn: (data: Uint8Array) => Uint8Array;

  constructor(decompressFn: (data: Uint8Array) => Uint8Array = decompress) {
    this.decompressFn = decompressFn;
  }

  draw(args: {
    cache: TileCache;
    hash: bigint;
    data: Uint8Array;
    rect: { x: number; y: number; w: number; h: number } | null;
    ctx: CanvasRenderingContext2D | null;
    glRenderer: WebGLTileRenderer | null;
    shouldDraw?: () => boolean;
  }): ZstdTileDrawResult {
    const { cache, hash, data, rect, ctx, glRenderer, shouldDraw } = args;
    if (!ctx && !glRenderer) {
      return { kind: 'skipped', reason: 'no-output' };
    }
    if (!rect) {
      return { kind: 'skipped', reason: 'offscreen' };
    }

    let decompressed: Uint8Array;
    try {
      decompressed = this.decompressFn(data);
    } catch {
      return { kind: 'miss', reason: 'decode-failed' };
    }

    const expectedBytes = rect.w * rect.h * 4;
    if (decompressed.length !== expectedBytes) {
      return { kind: 'miss', reason: 'size-mismatch' };
    }

    const pixels = decompressed.buffer instanceof ArrayBuffer
      ? new Uint8ClampedArray(
        decompressed.buffer,
        decompressed.byteOffset,
        decompressed.byteLength,
      )
      : Uint8ClampedArray.from(decompressed);
    const imageData = new ImageData(pixels, rect.w, rect.h);
    const cacheable = hash !== 0n;
    const redundant = cacheable && cache.has(hash);
    if (cacheable) {
      cache.set(hash, imageData);
    }

    if (shouldDraw && !shouldDraw()) {
      return {
        kind: 'cached',
        redundant,
        encodedBytes: data.byteLength,
      };
    }

    if (glRenderer) {
      glRenderer.drawTileImageData(rect.x, rect.y, rect.w, rect.h, imageData);
    } else {
      ctx!.putImageData(imageData, rect.x, rect.y);
    }

    return {
      kind: 'drawn',
      redundant,
      encodedBytes: data.byteLength,
    };
  }
}
