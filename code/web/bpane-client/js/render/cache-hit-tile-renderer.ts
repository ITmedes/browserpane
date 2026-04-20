import type { TileCache } from '../tile-cache.js';
import type { WebGLTileRenderer } from '../webgl-compositor.js';

export type CacheHitTileDrawResult =
  | { kind: 'drawn' }
  | { kind: 'miss'; reason: 'missing' | 'size-mismatch' }
  | { kind: 'skipped'; reason: 'offscreen' | 'no-output' };

export class CacheHitTileRenderer {
  draw(args: {
    cache: TileCache;
    hash: bigint;
    rect: { x: number; y: number; w: number; h: number } | null;
    ctx: CanvasRenderingContext2D | null;
    glRenderer: WebGLTileRenderer | null;
  }): CacheHitTileDrawResult {
    const { cache, hash, rect, ctx, glRenderer } = args;
    if (!ctx && !glRenderer) {
      return { kind: 'skipped', reason: 'no-output' };
    }
    if (!rect) {
      return { kind: 'skipped', reason: 'offscreen' };
    }

    const tile = cache.get(hash);
    if (!tile) {
      return { kind: 'miss', reason: 'missing' };
    }

    if (glRenderer) {
      if ('close' in tile && typeof tile.close === 'function') {
        glRenderer.drawTileImageBitmap(rect.x, rect.y, rect.w, rect.h, tile as ImageBitmap);
        return { kind: 'drawn' };
      }

      const imageData = tile as ImageData;
      if (imageData.width !== rect.w || imageData.height !== rect.h) {
        return { kind: 'miss', reason: 'size-mismatch' };
      }
      glRenderer.drawTileImageData(rect.x, rect.y, rect.w, rect.h, imageData);
      return { kind: 'drawn' };
    }

    const canvasContext = ctx!;
    if ('close' in tile && typeof tile.close === 'function') {
      canvasContext.drawImage(tile as ImageBitmap, rect.x, rect.y, rect.w, rect.h);
      return { kind: 'drawn' };
    }

    const imageData = tile as ImageData;
    if (imageData.width !== rect.w || imageData.height !== rect.h) {
      return { kind: 'miss', reason: 'size-mismatch' };
    }
    canvasContext.putImageData(imageData, rect.x, rect.y);
    return { kind: 'drawn' };
  }
}
