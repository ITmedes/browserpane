import { afterEach, describe, expect, it, vi } from 'vitest';

import { WebGLCachedVideoRenderer } from '../render/webgl-cached-video-renderer.js';

function createVideoGl(): WebGL2RenderingContext {
  let textureId = 0;

  return {
    TEXTURE_2D: 0x0DE1,
    TEXTURE_MIN_FILTER: 0x2801,
    TEXTURE_MAG_FILTER: 0x2800,
    LINEAR: 0x2601,
    CLAMP_TO_EDGE: 0x812F,
    TEXTURE_WRAP_S: 0x2802,
    TEXTURE_WRAP_T: 0x2803,
    RGBA: 0x1908,
    UNSIGNED_BYTE: 0x1401,
    SCISSOR_TEST: 0x0C11,
    TRIANGLES: 0x0004,
    createTexture: vi.fn(() => ({ id: `texture-${++textureId}` })),
    deleteTexture: vi.fn(),
    bindTexture: vi.fn(),
    texParameteri: vi.fn(),
    texImage2D: vi.fn(),
    useProgram: vi.fn(),
    bindVertexArray: vi.fn(),
    uniform4f: vi.fn(),
    uniform1i: vi.fn(),
    drawArrays: vi.fn(),
    enable: vi.fn(),
    scissor: vi.fn(),
    disable: vi.fn(),
  } as unknown as WebGL2RenderingContext;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('WebGLCachedVideoRenderer', () => {
  it('returns false for draw calls before any frame has been uploaded', () => {
    const gl = createVideoGl();
    const renderer = new WebGLCachedVideoRenderer(gl, {
      program: { id: 'program' } as unknown as WebGLProgram,
      vao: { id: 'vao' } as unknown as WebGLVertexArrayObject,
      uRect: { id: 'uRect' } as unknown as WebGLUniformLocation,
      uMode: { id: 'uMode' } as unknown as WebGLUniformLocation,
    });

    expect(renderer.draw(0, 0, 10, 10)).toBe(false);
    expect(renderer.drawCropped(0, 0, 100, 50, 50, 60, 70, 80)).toBe(false);
    expect(gl.drawArrays).not.toHaveBeenCalled();
  });

  it('uploads frames, reuses the texture for the same size, and recreates it on size changes', () => {
    const gl = createVideoGl() as any;
    const renderer = new WebGLCachedVideoRenderer(gl, {
      program: { id: 'program' } as unknown as WebGLProgram,
      vao: { id: 'vao' } as unknown as WebGLVertexArrayObject,
      uRect: { id: 'uRect' } as unknown as WebGLUniformLocation,
      uMode: { id: 'uMode' } as unknown as WebGLUniformLocation,
    });

    renderer.upload({ displayWidth: 640, displayHeight: 360 } as unknown as VideoFrame);
    renderer.upload({ displayWidth: 640, displayHeight: 360 } as unknown as VideoFrame);
    renderer.upload({ displayWidth: 320, displayHeight: 180 } as unknown as VideoFrame);

    expect(gl.createTexture).toHaveBeenCalledTimes(2);
    expect(gl.deleteTexture).toHaveBeenCalledWith({ id: 'texture-1' });
    expect(gl.texImage2D).toHaveBeenCalledTimes(3);
  });

  it('draws the cached texture and clipped destination rectangles after upload', () => {
    const gl = createVideoGl() as any;
    const renderer = new WebGLCachedVideoRenderer(gl, {
      program: { id: 'program' } as unknown as WebGLProgram,
      vao: { id: 'vao' } as unknown as WebGLVertexArrayObject,
      uRect: { id: 'uRect' } as unknown as WebGLUniformLocation,
      uMode: { id: 'uMode' } as unknown as WebGLUniformLocation,
    });
    renderer.resize(600);
    renderer.upload({ displayWidth: 640, displayHeight: 360 } as unknown as VideoFrame);

    expect(renderer.draw(10, 20, 30, 40)).toBe(true);
    expect(gl.uniform4f).toHaveBeenCalledWith({ id: 'uRect' }, 10, 20, 30, 40);

    expect(renderer.drawCropped(0, 0, 100, 50, 50, 60, 70, 80)).toBe(true);
    expect(gl.enable).toHaveBeenCalledWith(gl.SCISSOR_TEST);
    expect(gl.scissor).toHaveBeenCalledWith(50, 460, 70, 80);
    expect(gl.disable).toHaveBeenCalledWith(gl.SCISSOR_TEST);
  });

  it('invalidates and destroys the cached texture cleanly', () => {
    const gl = createVideoGl() as any;
    const renderer = new WebGLCachedVideoRenderer(gl, {
      program: { id: 'program' } as unknown as WebGLProgram,
      vao: { id: 'vao' } as unknown as WebGLVertexArrayObject,
      uRect: { id: 'uRect' } as unknown as WebGLUniformLocation,
      uMode: { id: 'uMode' } as unknown as WebGLUniformLocation,
    });

    renderer.upload({ displayWidth: 640, displayHeight: 360 } as unknown as VideoFrame);
    expect(renderer.draw(0, 0, 10, 10)).toBe(true);

    renderer.invalidate();
    expect(renderer.draw(0, 0, 10, 10)).toBe(false);

    renderer.destroy();
    expect(gl.deleteTexture).toHaveBeenCalledWith({ id: 'texture-1' });
    expect(renderer.draw(0, 0, 10, 10)).toBe(false);
  });
});
