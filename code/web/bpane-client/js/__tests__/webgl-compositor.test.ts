import { afterEach, describe, it, expect, vi } from 'vitest';
import { WebGLTileRenderer } from '../webgl-compositor.js';

function createProbeGl(opts: {
  renderer: string;
  vendor?: string;
  loseContext?: ReturnType<typeof vi.fn>;
}): WebGL2RenderingContext {
  const loseContext = opts.loseContext ?? vi.fn();
  const debugInfo = {
    UNMASKED_RENDERER_WEBGL: 0x9246,
    UNMASKED_VENDOR_WEBGL: 0x9245,
  };

  return {
    RENDERER: 0x1F01,
    VENDOR: 0x1F00,
    getExtension(name: string) {
      if (name === 'WEBGL_debug_renderer_info') return debugInfo;
      if (name === 'WEBGL_lose_context') return { loseContext };
      return null;
    },
    getParameter(param: number) {
      if (param === debugInfo.UNMASKED_RENDERER_WEBGL) return opts.renderer;
      if (param === debugInfo.UNMASKED_VENDOR_WEBGL) return opts.vendor ?? 'Google Inc.';
      if (param === 0x1F01) return 'WebGL Renderer';
      if (param === 0x1F00) return opts.vendor ?? 'Google Inc.';
      return null;
    },
  } as unknown as WebGL2RenderingContext;
}

function createHardwareGl(renderer: string): WebGL2RenderingContext {
  return {
    ...createProbeGl({ renderer }),
    VERTEX_SHADER: 0x8B31,
    FRAGMENT_SHADER: 0x8B30,
    COMPILE_STATUS: 0x8B81,
    LINK_STATUS: 0x8B82,
    ARRAY_BUFFER: 0x8892,
    STATIC_DRAW: 0x88E4,
    FLOAT: 0x1406,
    TEXTURE_2D: 0x0DE1,
    TEXTURE_MIN_FILTER: 0x2801,
    TEXTURE_MAG_FILTER: 0x2800,
    NEAREST: 0x2600,
    LINEAR: 0x2601,
    TEXTURE_WRAP_S: 0x2802,
    TEXTURE_WRAP_T: 0x2803,
    CLAMP_TO_EDGE: 0x812F,
    BLEND: 0x0BE2,
    DEPTH_TEST: 0x0B71,
    SCISSOR_TEST: 0x0C11,
    COLOR_BUFFER_BIT: 0x4000,
    RGBA: 0x1908,
    UNSIGNED_BYTE: 0x1401,
    RGBA8: 0x8058,
    COLOR_ATTACHMENT0: 0x8CE0,
    FRAMEBUFFER: 0x8D40,
    READ_FRAMEBUFFER: 0x8CA8,
    DRAW_FRAMEBUFFER: 0x8CA9,
    TRIANGLES: 0x0004,
    createShader: vi.fn(() => ({})),
    shaderSource: vi.fn(),
    compileShader: vi.fn(),
    getShaderParameter: vi.fn((_shader, param) => param === 0x8B81),
    getShaderInfoLog: vi.fn(() => ''),
    deleteShader: vi.fn(),
    createProgram: vi.fn(() => ({})),
    attachShader: vi.fn(),
    linkProgram: vi.fn(),
    getProgramParameter: vi.fn((_program, param) => param === 0x8B82),
    getProgramInfoLog: vi.fn(() => ''),
    deleteProgram: vi.fn(),
    useProgram: vi.fn(),
    getUniformLocation: vi.fn(() => ({})),
    createVertexArray: vi.fn(() => ({})),
    bindVertexArray: vi.fn(),
    createBuffer: vi.fn(() => ({})),
    bindBuffer: vi.fn(),
    bufferData: vi.fn(),
    getAttribLocation: vi.fn((_program, name) => (name === 'a_position' ? 0 : 1)),
    enableVertexAttribArray: vi.fn(),
    vertexAttribPointer: vi.fn(),
    createTexture: vi.fn(() => ({})),
    bindTexture: vi.fn(),
    texParameteri: vi.fn(),
    disable: vi.fn(),
    clearColor: vi.fn(),
    viewport: vi.fn(),
    uniform2f: vi.fn(),
    uniform4f: vi.fn(),
    uniform1i: vi.fn(),
    drawArrays: vi.fn(),
    createFramebuffer: vi.fn(() => ({})),
    bindFramebuffer: vi.fn(),
    framebufferTexture2D: vi.fn(),
    deleteFramebuffer: vi.fn(),
    deleteTexture: vi.fn(),
    deleteBuffer: vi.fn(),
    deleteVertexArray: vi.fn(),
    blitFramebuffer: vi.fn(),
    clear: vi.fn(),
    enable: vi.fn(),
    scissor: vi.fn(),
    texImage2D: vi.fn(),
  } as unknown as WebGL2RenderingContext;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('WebGLTileRenderer.tryCreate', () => {
  it('requests a strict WebGL2 context first', () => {
    const strictGetContext = vi.fn(() => null);
    const probeGetContext = vi.fn(() => null);
    const canvas = {
      getContext: strictGetContext,
      ownerDocument: {
        createElement: vi.fn(() => ({ getContext: probeGetContext })),
      },
    } as unknown as HTMLCanvasElement;

    const result = WebGLTileRenderer.tryCreate(canvas);

    expect(result.renderer).toBeNull();
    expect(strictGetContext).toHaveBeenCalledWith('webgl2', expect.objectContaining({
      failIfMajorPerformanceCaveat: true,
      powerPreference: 'high-performance',
    }));
  });

  it('falls back when only a software WebGL renderer is available', () => {
    const loseContext = vi.fn();
    const softwareGl = createProbeGl({
      renderer: 'ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)), SwiftShader driver)',
      loseContext,
    });
    const probeGetContext = vi.fn(() => softwareGl);
    const canvas = {
      getContext: vi.fn(() => null),
      ownerDocument: {
        createElement: vi.fn(() => ({ getContext: probeGetContext })),
      },
    } as unknown as HTMLCanvasElement;

    const result = WebGLTileRenderer.tryCreate(canvas);

    expect(result.renderer).toBeNull();
    expect(result.diagnostics.backend).toBe('canvas2d');
    expect(result.diagnostics.reason).toBe('software-renderer');
    expect(result.diagnostics.software).toBe(true);
    expect(result.diagnostics.renderer).toContain('SwiftShader');
    expect(loseContext).toHaveBeenCalledOnce();
  });

  it('reports unsupported when neither strict nor probe contexts are available', () => {
    const canvas = {
      getContext: vi.fn(() => null),
      ownerDocument: {
        createElement: vi.fn(() => ({ getContext: vi.fn(() => null) })),
      },
    } as unknown as HTMLCanvasElement;

    const result = WebGLTileRenderer.tryCreate(canvas);

    expect(result.renderer).toBeNull();
    expect(result.diagnostics).toEqual({
      backend: 'canvas2d',
      renderer: null,
      vendor: null,
      software: false,
      reason: 'unsupported',
    });
  });

  it('reports major-performance-caveat when strict context creation fails but hardware probe works', () => {
    const loseContext = vi.fn();
    const probeGl = createProbeGl({
      renderer: 'ANGLE (AMD Radeon RX 7900 XT Direct3D11)',
      vendor: 'AMD',
      loseContext,
    });
    const canvas = {
      getContext: vi.fn(() => null),
      ownerDocument: {
        createElement: vi.fn(() => ({ getContext: vi.fn(() => probeGl) })),
      },
    } as unknown as HTMLCanvasElement;

    const result = WebGLTileRenderer.tryCreate(canvas);

    expect(result.renderer).toBeNull();
    expect(result.diagnostics.reason).toBe('major-performance-caveat');
    expect(result.diagnostics.software).toBe(false);
    expect(result.diagnostics.renderer).toContain('Radeon');
    expect(loseContext).toHaveBeenCalledOnce();
  });

  it('reports initialization-failed when context creation throws', () => {
    const canvas = {
      getContext: vi.fn(() => {
        throw new Error('driver init failed');
      }),
    } as unknown as HTMLCanvasElement;

    const result = WebGLTileRenderer.tryCreate(canvas);

    expect(result.renderer).toBeNull();
    expect(result.diagnostics.reason).toBe('initialization-failed');
  });

  it('keeps the WebGL2 path when a hardware renderer is available', () => {
    const gl = createHardwareGl('ANGLE (NVIDIA RTX 4090 Direct3D11 vs_5_0 ps_5_0)');
    const canvas = {
      getContext: vi.fn(() => gl),
      ownerDocument: {
        createElement: vi.fn(),
      },
    } as unknown as HTMLCanvasElement;

    const result = WebGLTileRenderer.tryCreate(canvas);

    expect(result.renderer).toBeInstanceOf(WebGLTileRenderer);
    expect(result.diagnostics.backend).toBe('webgl2');
    expect(result.diagnostics.reason).toBe('hardware-accelerated');
    expect(result.diagnostics.software).toBe(false);
    expect(result.renderer?.getContextInfo()).toEqual(expect.objectContaining({
      renderer: 'ANGLE (NVIDIA RTX 4090 Direct3D11 vs_5_0 ps_5_0)',
      software: false,
    }));
  });
});

describe('WebGLTileRenderer', () => {
  it('resizes and draws fills, ImageData, ImageBitmaps, and VideoFrames', () => {
    const gl = createHardwareGl('ANGLE (NVIDIA RTX 4090 Direct3D11 vs_5_0 ps_5_0)') as any;
    const renderer = new WebGLTileRenderer(gl);

    renderer.resize(800, 600);
    expect(gl.viewport).toHaveBeenCalledWith(0, 0, 800, 600);
    expect(gl.uniform2f).toHaveBeenCalledWith(expect.anything(), 800, 600);

    renderer.drawFill(10, 20, 30, 40, 255, 128, 64, 0.5);
    const fillCalls = gl.uniform4f.mock.calls;
    expect(fillCalls.at(-2).slice(1)).toEqual([10, 20, 30, 40]);
    expect(fillCalls.at(-1)[1]).toBeCloseTo(1);
    expect(fillCalls.at(-1)[2]).toBeCloseTo(128 / 255);
    expect(fillCalls.at(-1)[3]).toBeCloseTo(64 / 255);
    expect(fillCalls.at(-1)[4]).toBeCloseTo(0.5);

    const imageData = {
      width: 2,
      height: 2,
      data: new Uint8ClampedArray(16),
    } as unknown as ImageData;
    renderer.drawTileImageData(1, 2, 3, 4, imageData);
    expect(gl.texImage2D.mock.calls.at(-1)).toEqual([
      gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, imageData,
    ]);

    const bitmap = { width: 2, height: 2 } as unknown as ImageBitmap;
    renderer.drawTileImageBitmap(5, 6, 7, 8, bitmap);
    expect(gl.texImage2D.mock.calls.at(-1)).toEqual([
      gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, bitmap,
    ]);

    const frame = { displayWidth: 320, displayHeight: 180 } as unknown as VideoFrame;
    renderer.drawVideoFrame(9, 10, 11, 12, frame);
    expect(gl.texImage2D.mock.calls.at(-1)).toEqual([
      gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, frame,
    ]);
  });

  it('manages cached video textures, cropping, and invalidation', () => {
    const gl = createHardwareGl('ANGLE (NVIDIA RTX 4090 Direct3D11 vs_5_0 ps_5_0)') as any;
    const renderer = new WebGLTileRenderer(gl);
    renderer.resize(800, 600);

    expect(renderer.drawCachedVideo(0, 0, 10, 10)).toBe(false);

    const initialTextureCreates = gl.createTexture.mock.calls.length;
    renderer.uploadVideoFrame({ displayWidth: 640, displayHeight: 360 } as unknown as VideoFrame);
    expect(gl.createTexture).toHaveBeenCalledTimes(initialTextureCreates + 1);

    expect(renderer.drawCachedVideo(10, 20, 30, 40)).toBe(true);
    expect(gl.uniform4f.mock.calls.at(-1).slice(1)).toEqual([10, 20, 30, 40]);

    expect(renderer.drawCachedVideoCropped(0, 0, 100, 50, 50, 60, 70, 80)).toBe(true);
    expect(gl.enable).toHaveBeenCalledWith(gl.SCISSOR_TEST);
    expect(gl.scissor).toHaveBeenCalledWith(50, 460, 70, 80);

    renderer.invalidateVideoTexture();
    expect(renderer.drawCachedVideo(0, 0, 10, 10)).toBe(false);

    renderer.uploadVideoFrame({ displayWidth: 320, displayHeight: 180 } as unknown as VideoFrame);
    expect(gl.deleteTexture).toHaveBeenCalledTimes(1);
  });

  it('draws cropped sources, scroll-copies regions, clears, and destroys resources', () => {
    const gl = createHardwareGl('ANGLE (NVIDIA RTX 4090 Direct3D11 vs_5_0 ps_5_0)') as any;
    const renderer = new WebGLTileRenderer(gl);
    renderer.resize(640, 480);

    const source = {} as TexImageSource;
    renderer.drawTexImageSourceCropped(source, 10, 20, 30, 40, 50, 60, 70, 80, 300, 200);
    expect(gl.scissor).toHaveBeenCalledWith(50, 340, 70, 80);
    const croppedRect = gl.uniform4f.mock.calls.at(-1);
    expect(croppedRect[1]).toBeCloseTo(26.6666667);
    expect(croppedRect[2]).toBeCloseTo(20);
    expect(croppedRect[3]).toBeCloseTo(700);
    expect(croppedRect[4]).toBeCloseTo(400);

    renderer.scrollCopy(0, -64, 64, 320, 400, 640, 480);
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

    renderer.clear();
    expect(gl.clear).toHaveBeenCalledWith(gl.COLOR_BUFFER_BIT);

    renderer.destroy();
    expect(gl.deleteFramebuffer).toHaveBeenCalledTimes(1);
    expect(gl.deleteTexture).toHaveBeenCalledTimes(2);
    expect(gl.deleteBuffer).toHaveBeenCalledTimes(1);
    expect(gl.deleteVertexArray).toHaveBeenCalledTimes(1);
    expect(gl.deleteProgram).toHaveBeenCalledTimes(1);
  });
});
