import type { WebGLTileRenderer } from '../webgl-compositor.js';

export type FillTileDrawResult =
  | { kind: 'drawn' }
  | { kind: 'skipped'; reason: 'offscreen' | 'no-output' };

export class FillTileRenderer {
  private lastCanvasCtx: CanvasRenderingContext2D | null = null;
  private lastCanvasRgba: number | null = null;
  private lastCanvasFillStyle = '';

  draw(args: {
    rect: { x: number; y: number; w: number; h: number } | null;
    rgba: number;
    ctx: CanvasRenderingContext2D | null;
    glRenderer: WebGLTileRenderer | null;
  }): FillTileDrawResult {
    const { rect, rgba, ctx, glRenderer } = args;
    if (!ctx && !glRenderer) {
      return { kind: 'skipped', reason: 'no-output' };
    }
    if (!rect) {
      return { kind: 'skipped', reason: 'offscreen' };
    }

    const r = (rgba >>> 0) & 0xff;
    const g = (rgba >>> 8) & 0xff;
    const b = (rgba >>> 16) & 0xff;
    const a = ((rgba >>> 24) & 0xff) / 255;

    if (glRenderer) {
      glRenderer.drawFill(rect.x, rect.y, rect.w, rect.h, r, g, b, a);
    } else {
      const canvasContext = ctx!;
      if (canvasContext !== this.lastCanvasCtx || rgba !== this.lastCanvasRgba) {
        this.lastCanvasFillStyle = `rgba(${r},${g},${b},${a})`;
        canvasContext.fillStyle = this.lastCanvasFillStyle;
        this.lastCanvasCtx = canvasContext;
        this.lastCanvasRgba = rgba;
      }
      canvasContext.fillRect(rect.x, rect.y, rect.w, rect.h);
    }

    return { kind: 'drawn' };
  }
}
