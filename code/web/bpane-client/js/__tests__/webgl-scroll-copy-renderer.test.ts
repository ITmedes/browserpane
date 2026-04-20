import { afterEach, describe, expect, it, vi } from 'vitest';

import { WebGLScrollCopyRenderer } from '../render/webgl-scroll-copy-renderer.js';

function createScrollGl(): WebGL2RenderingContext {
  let textureId = 0;
  let framebufferId = 0;

  return {
    TEXTURE_2D: 0x0DE1,
    TEXTURE_MIN_FILTER: 0x2801,
    TEXTURE_MAG_FILTER: 0x2800,
    NEAREST: 0x2600,
    TEXTURE_WRAP_S: 0x2802,
    TEXTURE_WRAP_T: 0x2803,
    CLAMP_TO_EDGE: 0x812F,
    RGBA8: 0x8058,
    RGBA: 0x1908,
    UNSIGNED_BYTE: 0x1401,
    FRAMEBUFFER: 0x8D40,
    COLOR_ATTACHMENT0: 0x8CE0,
    READ_FRAMEBUFFER: 0x8CA8,
    DRAW_FRAMEBUFFER: 0x8CA9,
    COLOR_BUFFER_BIT: 0x4000,
    createTexture: vi.fn(() => ({ id: `texture-${++textureId}` })),
    bindTexture: vi.fn(),
    texImage2D: vi.fn(),
    texParameteri: vi.fn(),
    deleteTexture: vi.fn(),
    createFramebuffer: vi.fn(() => ({ id: `framebuffer-${++framebufferId}` })),
    bindFramebuffer: vi.fn(),
    framebufferTexture2D: vi.fn(),
    deleteFramebuffer: vi.fn(),
    blitFramebuffer: vi.fn(),
  } as unknown as WebGL2RenderingContext;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('WebGLScrollCopyRenderer', () => {
  it('copies a scroll region by blitting the cached framebuffer and shifted repair area', () => {
    const gl = createScrollGl() as any;
    const renderer = new WebGLScrollCopyRenderer(gl);

    renderer.scrollCopy({
      canvasWidth: 640,
      canvasHeight: 480,
      dx: 0,
      dy: -64,
      regionTop: 64,
      regionBottom: 320,
      regionRight: 400,
      screenW: 640,
      screenH: 480,
    });

    expect(gl.createTexture).toHaveBeenCalledTimes(1);
    expect(gl.createFramebuffer).toHaveBeenCalledTimes(1);
    expect(gl.blitFramebuffer.mock.calls[0]).toEqual([
      0, 160, 400, 416,
      0, 160, 400, 416,
      gl.COLOR_BUFFER_BIT,
      gl.NEAREST,
    ]);
    expect(gl.blitFramebuffer.mock.calls[1]).toEqual([
      0, 224, 400, 416,
      0, 160, 400, 352,
      gl.COLOR_BUFFER_BIT,
      gl.NEAREST,
    ]);
  });

  it('handles full-screen scroll copies with translated source and destination rectangles', () => {
    const gl = createScrollGl() as any;
    const renderer = new WebGLScrollCopyRenderer(gl);

    renderer.scrollCopy({
      canvasWidth: 640,
      canvasHeight: 480,
      dx: 20,
      dy: -10,
      regionTop: 0,
      regionBottom: 480,
      regionRight: 640,
      screenW: 640,
      screenH: 480,
    });

    expect(gl.blitFramebuffer.mock.calls[0]).toEqual([
      0, 0, 640, 480,
      0, 0, 640, 480,
      gl.COLOR_BUFFER_BIT,
      gl.NEAREST,
    ]);
    expect(gl.blitFramebuffer.mock.calls[1]).toEqual([
      20, 10, 640, 480,
      0, 0, 620, 470,
      gl.COLOR_BUFFER_BIT,
      gl.NEAREST,
    ]);
  });

  it('skips scroll-copy work when the canvas size is empty', () => {
    const gl = createScrollGl() as any;
    const renderer = new WebGLScrollCopyRenderer(gl);

    renderer.scrollCopy({
      canvasWidth: 0,
      canvasHeight: 480,
      dx: 0,
      dy: -64,
      regionTop: 64,
      regionBottom: 320,
      regionRight: 400,
      screenW: 640,
      screenH: 480,
    });

    expect(gl.createTexture).not.toHaveBeenCalled();
    expect(gl.createFramebuffer).not.toHaveBeenCalled();
    expect(gl.blitFramebuffer).not.toHaveBeenCalled();
  });

  it('reuses scroll resources for the same size, recreates them on resize, and cleans up on destroy', () => {
    const gl = createScrollGl() as any;
    const renderer = new WebGLScrollCopyRenderer(gl);

    renderer.scrollCopy({
      canvasWidth: 640,
      canvasHeight: 480,
      dx: 0,
      dy: -64,
      regionTop: 64,
      regionBottom: 320,
      regionRight: 400,
      screenW: 640,
      screenH: 480,
    });
    renderer.scrollCopy({
      canvasWidth: 640,
      canvasHeight: 480,
      dx: 0,
      dy: -32,
      regionTop: 64,
      regionBottom: 320,
      regionRight: 400,
      screenW: 640,
      screenH: 480,
    });

    expect(gl.createTexture).toHaveBeenCalledTimes(1);
    expect(gl.createFramebuffer).toHaveBeenCalledTimes(1);

    renderer.scrollCopy({
      canvasWidth: 800,
      canvasHeight: 600,
      dx: 0,
      dy: -32,
      regionTop: 64,
      regionBottom: 320,
      regionRight: 400,
      screenW: 800,
      screenH: 600,
    });

    expect(gl.deleteTexture).toHaveBeenCalledWith({ id: 'texture-1' });
    expect(gl.deleteFramebuffer).toHaveBeenCalledWith({ id: 'framebuffer-1' });

    renderer.destroy();
    expect(gl.deleteTexture).toHaveBeenCalledWith({ id: 'texture-2' });
    expect(gl.deleteFramebuffer).toHaveBeenCalledWith({ id: 'framebuffer-2' });
  });
});
