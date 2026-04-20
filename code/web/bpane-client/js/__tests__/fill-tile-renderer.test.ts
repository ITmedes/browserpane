import { describe, expect, it, vi } from 'vitest';

import { FillTileRenderer } from '../render/fill-tile-renderer.js';

function createCanvasContext(): CanvasRenderingContext2D & { fillStyleAssignments: number } {
  let fillStyle = '';
  let fillStyleAssignments = 0;
  return {
    get fillStyleAssignments() {
      return fillStyleAssignments;
    },
    get fillStyle() {
      return fillStyle;
    },
    set fillStyle(value: string) {
      fillStyle = value;
      fillStyleAssignments++;
    },
    fillRect: vi.fn(),
  } as unknown as CanvasRenderingContext2D & { fillStyleAssignments: number };
}

function createWebGLRenderer() {
  return {
    drawFill: vi.fn(),
  };
}

describe('FillTileRenderer', () => {
  it('draws a fill to the canvas context with rgba conversion', () => {
    const ctx = createCanvasContext();
    const renderer = new FillTileRenderer();

    const result = renderer.draw({
      rect: { x: 10, y: 20, w: 30, h: 40 },
      rgba: 0xff332211,
      ctx,
      glRenderer: null,
    });

    expect(result).toEqual({ kind: 'drawn' });
    expect(ctx.fillStyle).toBe('rgba(17,34,51,1)');
    expect(ctx.fillRect).toHaveBeenCalledWith(10, 20, 30, 40);
  });

  it('draws a fill through the WebGL renderer with normalized alpha', () => {
    const glRenderer = createWebGLRenderer();
    const renderer = new FillTileRenderer();

    const result = renderer.draw({
      rect: { x: 5, y: 6, w: 7, h: 8 },
      rgba: 0x80443322,
      ctx: null,
      glRenderer: glRenderer as any,
    });

    expect(result).toEqual({ kind: 'drawn' });
    expect(glRenderer.drawFill).toHaveBeenCalledWith(5, 6, 7, 8, 34, 51, 68, 128 / 255);
  });

  it('reuses the same canvas fillStyle assignment for consecutive identical colors', () => {
    const ctx = createCanvasContext();
    const renderer = new FillTileRenderer();

    renderer.draw({
      rect: { x: 1, y: 2, w: 3, h: 4 },
      rgba: 0xff332211,
      ctx,
      glRenderer: null,
    });
    renderer.draw({
      rect: { x: 10, y: 20, w: 30, h: 40 },
      rgba: 0xff332211,
      ctx,
      glRenderer: null,
    });

    expect(ctx.fillStyle).toBe('rgba(17,34,51,1)');
    expect(ctx.fillStyleAssignments).toBe(1);
    expect(ctx.fillRect).toHaveBeenCalledTimes(2);
  });

  it('skips drawing when the tile is offscreen', () => {
    const renderer = new FillTileRenderer();

    expect(renderer.draw({
      rect: null,
      rgba: 0xff,
      ctx: createCanvasContext(),
      glRenderer: null,
    })).toEqual({ kind: 'skipped', reason: 'offscreen' });
  });

  it('skips drawing when there is no active output', () => {
    const renderer = new FillTileRenderer();

    expect(renderer.draw({
      rect: { x: 0, y: 0, w: 64, h: 64 },
      rgba: 0xff,
      ctx: null,
      glRenderer: null,
    })).toEqual({ kind: 'skipped', reason: 'no-output' });
  });
});
