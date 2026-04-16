import { decompress } from 'fzstd';

import type { TileCache } from '../tile-cache.js';
import type { WebGLTileRenderer } from '../webgl-compositor.js';

export type ZstdTileDrawResult =
  | { kind: 'drawn'; redundant: boolean; encodedBytes: number }
  | { kind: 'cached'; redundant: boolean; encodedBytes: number }
  | { kind: 'miss'; reason: 'decode-failed' | 'size-mismatch' }
  | { kind: 'skipped'; reason: 'offscreen' | 'no-output' };

export class ZstdTileRenderer {
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
      decompressed = decompress(data);
    } catch {
      return { kind: 'miss', reason: 'decode-failed' };
    }

    const expectedBytes = rect.w * rect.h * 4;
    if (decompressed.length !== expectedBytes) {
      return { kind: 'miss', reason: 'size-mismatch' };
    }

    const imageData = new ImageData(new Uint8ClampedArray(decompressed), rect.w, rect.h);
    const redundant = cache.has(hash);
    cache.set(hash, imageData);

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
