import { describe, expect, it, vi } from 'vitest';
import { CanvasScrollCopyRenderer } from '../render/canvas-scroll-copy-renderer.js';

function createMainContext() {
  const canvas = { width: 640, height: 640 };
  const ctx = {
    canvas,
    drawImage: vi.fn(),
    clearRect: vi.fn(),
  } as unknown as CanvasRenderingContext2D;
  return { canvas, ctx };
}

function createScratchCanvas(scratchCtx: CanvasRenderingContext2D | null) {
  return {
    width: 0,
    height: 0,
    getContext: vi.fn(() => scratchCtx),
  } as unknown as HTMLCanvasElement;
}

function createScratchContext() {
  return {
    clearRect: vi.fn(),
    drawImage: vi.fn(),
  } as unknown as CanvasRenderingContext2D;
}

describe('CanvasScrollCopyRenderer', () => {
  it('reuses a scratch canvas for full-screen scroll copies', () => {
    const { canvas, ctx } = createMainContext();
    const scratchCtx = createScratchContext();
    const scratchCanvas = createScratchCanvas(scratchCtx);
    const renderer = new CanvasScrollCopyRenderer(() => scratchCanvas);

    renderer.apply({
      ctx,
      dx: 0,
      dy: -128,
      regionTop: 0,
      regionBottom: 640,
      regionRight: 640,
      screenW: 640,
      screenH: 640,
    });

    expect(scratchCanvas.width).toBe(640);
    expect(scratchCanvas.height).toBe(640);
    expect(scratchCtx.clearRect).toHaveBeenCalledWith(0, 0, 640, 640);
    expect(scratchCtx.drawImage).toHaveBeenCalledWith(canvas, 0, 0);
    expect(ctx.drawImage).toHaveBeenCalledWith(scratchCanvas, 0, 128);
  });

  it('clips viewport-only scroll copies to the shifted destination region', () => {
    const { ctx } = createMainContext();
    const scratchCtx = createScratchContext();
    const scratchCanvas = createScratchCanvas(scratchCtx);
    const renderer = new CanvasScrollCopyRenderer(() => scratchCanvas);

    renderer.apply({
      ctx,
      dx: 0,
      dy: -64,
      regionTop: 64,
      regionBottom: 320,
      regionRight: 400,
      screenW: 640,
      screenH: 640,
    });

    expect(ctx.drawImage).toHaveBeenCalledWith(
      scratchCanvas,
      0, 0, 400, 192,
      0, 128, 400, 192,
    );
  });

  it('does not crash when the scratch canvas has no 2d context', () => {
    const { ctx } = createMainContext();
    const scratchCanvas = createScratchCanvas(null);
    const renderer = new CanvasScrollCopyRenderer(() => scratchCanvas);

    renderer.apply({
      ctx,
      dx: 0,
      dy: -64,
      regionTop: 64,
      regionBottom: 320,
      regionRight: 400,
      screenW: 640,
      screenH: 640,
    });

    expect(ctx.drawImage).not.toHaveBeenCalled();
  });
});
