import { decodeQoi } from '../qoi.js';
import type { TileCache } from '../tile-cache.js';
import type { WebGLTileRenderer } from '../webgl-compositor.js';

export type QoiTileDrawResult =
  | { kind: 'drawn'; redundant: boolean; decodedBytes: number }
  | { kind: 'cached'; redundant: boolean; decodedBytes: number }
  | { kind: 'miss'; reason: 'decode-failed' | 'size-mismatch' }
  | { kind: 'skipped'; reason: 'offscreen' | 'no-output' };

export class QoiTileRenderer {
  draw(args: {
    cache: TileCache;
    hash: bigint;
    data: Uint8Array;
    rect: { x: number; y: number; w: number; h: number } | null;
    ctx: CanvasRenderingContext2D | null;
    glRenderer: WebGLTileRenderer | null;
    shouldDraw?: () => boolean;
  }): QoiTileDrawResult {
    const { cache, hash, data, rect, ctx, glRenderer, shouldDraw } = args;
    if (!ctx && !glRenderer) {
      return { kind: 'skipped', reason: 'no-output' };
    }
    if (!rect) {
      return { kind: 'skipped', reason: 'offscreen' };
    }

    const decoded = decodeQoi(data);
    if (!decoded) {
      return { kind: 'miss', reason: 'decode-failed' };
    }
    if (decoded.width !== rect.w || decoded.height !== rect.h) {
      return { kind: 'miss', reason: 'size-mismatch' };
    }

    const imageData = new ImageData(decoded.pixels, decoded.width, decoded.height);
    const cacheable = hash !== 0n;
    const redundant = cacheable && cache.has(hash);
    if (cacheable) {
      cache.set(hash, imageData);
    }

    if (shouldDraw && !shouldDraw()) {
      return {
        kind: 'cached',
        redundant,
        decodedBytes: data.byteLength,
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
      decodedBytes: data.byteLength,
    };
  }
}
