import { afterEach, describe, expect, it, vi } from 'vitest';

import { WebGLTextureSourceRenderer } from '../render/webgl-texture-source-renderer.js';

function createTextureGl(): WebGL2RenderingContext {
  let textureId = 0;

  return {
    TEXTURE_2D: 0x0DE1,
    TEXTURE_MIN_FILTER: 0x2801,
    TEXTURE_MAG_FILTER: 0x2800,
    NEAREST: 0x2600,
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

describe('WebGLTextureSourceRenderer', () => {
  it('creates a reusable texture and draws uploaded sources at the requested rect', () => {
    const gl = createTextureGl() as any;
    const renderer = new WebGLTextureSourceRenderer(gl, {
      program: { id: 'program' } as unknown as WebGLProgram,
      vao: { id: 'vao' } as unknown as WebGLVertexArrayObject,
      uRect: { id: 'uRect' } as unknown as WebGLUniformLocation,
      uMode: { id: 'uMode' } as unknown as WebGLUniformLocation,
    });

    const source = { width: 2, height: 2 } as unknown as ImageBitmap;
    renderer.draw(10, 20, 30, 40, source);

    expect(gl.createTexture).toHaveBeenCalledTimes(1);
    expect(gl.texParameteri).toHaveBeenCalledWith(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    expect(gl.texParameteri).toHaveBeenCalledWith(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    expect(gl.texParameteri).toHaveBeenCalledWith(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    expect(gl.texParameteri).toHaveBeenCalledWith(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    expect(gl.texImage2D).toHaveBeenCalledWith(
      gl.TEXTURE_2D,
      0,
      gl.RGBA,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      source,
    );
    expect(gl.uniform4f).toHaveBeenCalledWith({ id: 'uRect' }, 10, 20, 30, 40);
    expect(gl.uniform1i).toHaveBeenCalledWith({ id: 'uMode' }, 0);
  });

  it('draws cropped sources by expanding to the full rect and clipping with scissor', () => {
    const gl = createTextureGl() as any;
    const renderer = new WebGLTextureSourceRenderer(gl, {
      program: { id: 'program' } as unknown as WebGLProgram,
      vao: { id: 'vao' } as unknown as WebGLVertexArrayObject,
      uRect: { id: 'uRect' } as unknown as WebGLUniformLocation,
      uMode: { id: 'uMode' } as unknown as WebGLUniformLocation,
    });
    renderer.resize(480);

    renderer.drawCropped(
      {} as TexImageSource,
      10,
      20,
      30,
      40,
      50,
      60,
      70,
      80,
      300,
      200,
    );

    expect(gl.scissor).toHaveBeenCalledWith(50, 340, 70, 80);
    expect(gl.uniform4f).toHaveBeenCalledWith(
      { id: 'uRect' },
      expect.closeTo(26.6666667, 5),
      20,
      700,
      400,
    );
    expect(gl.disable).toHaveBeenCalledWith(gl.SCISSOR_TEST);
  });

  it('reuses the same texture across draws and deletes it during cleanup', () => {
    const gl = createTextureGl() as any;
    const renderer = new WebGLTextureSourceRenderer(gl, {
      program: { id: 'program' } as unknown as WebGLProgram,
      vao: { id: 'vao' } as unknown as WebGLVertexArrayObject,
      uRect: { id: 'uRect' } as unknown as WebGLUniformLocation,
      uMode: { id: 'uMode' } as unknown as WebGLUniformLocation,
    });

    renderer.draw(0, 0, 10, 10, { width: 1, height: 1 } as unknown as ImageBitmap);
    renderer.draw(5, 6, 7, 8, { width: 2, height: 2 } as unknown as ImageBitmap);

    expect(gl.createTexture).toHaveBeenCalledTimes(1);

    renderer.destroy();
    expect(gl.deleteTexture).toHaveBeenCalledWith({ id: 'texture-1' });
  });
});
