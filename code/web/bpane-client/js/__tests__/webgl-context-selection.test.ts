import { afterEach, describe, expect, it, vi } from 'vitest';

import { selectWebGLContext } from '../render/webgl-context-selection.js';

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
      if (param === 0x1F01) return 'Masked Renderer';
      if (param === 0x1F00) return opts.vendor ?? 'Google Inc.';
      return null;
    },
  } as unknown as WebGL2RenderingContext;
}

function createCanvas(opts: {
  strictContext: WebGL2RenderingContext | null;
  probeContext?: WebGL2RenderingContext | null;
  strictError?: Error;
}): HTMLCanvasElement {
  const strictGetContext = vi.fn(() => {
    if (opts.strictError) throw opts.strictError;
    return opts.strictContext;
  });
  const probeGetContext = vi.fn(() => opts.probeContext ?? null);

  return {
    getContext: strictGetContext,
    ownerDocument: {
      createElement: vi.fn(() => ({
        getContext: probeGetContext,
      })),
    },
  } as unknown as HTMLCanvasElement;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('selectWebGLContext', () => {
  it('returns a hardware-accelerated WebGL2 selection when strict context creation succeeds', () => {
    const gl = createProbeGl({
      renderer: 'ANGLE (Apple, ANGLE Metal Renderer: Apple M4 Pro, Unspecified Version)',
      vendor: 'Google Inc. (Apple)',
    });
    const canvas = createCanvas({ strictContext: gl });

    const result = selectWebGLContext(canvas);

    expect(result.gl).toBe(gl);
    expect(result.diagnostics).toEqual({
      backend: 'webgl2',
      renderer: 'ANGLE (Apple, ANGLE Metal Renderer: Apple M4 Pro, Unspecified Version)',
      vendor: 'Google Inc. (Apple)',
      software: false,
      reason: 'hardware-accelerated',
    });
  });

  it('falls back to canvas diagnostics when strict selection only finds a software renderer', () => {
    const loseContext = vi.fn();
    const gl = createProbeGl({
      renderer: 'ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)), SwiftShader driver)',
      loseContext,
    });
    const canvas = createCanvas({ strictContext: gl });

    const result = selectWebGLContext(canvas);

    expect(result.gl).toBeNull();
    expect(result.diagnostics.reason).toBe('software-renderer');
    expect(result.diagnostics.software).toBe(true);
    expect(result.diagnostics.renderer).toContain('SwiftShader');
    expect(loseContext).toHaveBeenCalledOnce();
  });

  it('reports a major performance caveat when strict creation fails but probe creation sees hardware', () => {
    const loseContext = vi.fn();
    const probeGl = createProbeGl({
      renderer: 'ANGLE (AMD Radeon RX 7900 XT Direct3D11)',
      vendor: 'AMD',
      loseContext,
    });
    const canvas = createCanvas({
      strictContext: null,
      probeContext: probeGl,
    });

    const result = selectWebGLContext(canvas);

    expect(result.gl).toBeNull();
    expect(result.diagnostics).toEqual({
      backend: 'canvas2d',
      renderer: 'ANGLE (AMD Radeon RX 7900 XT Direct3D11)',
      vendor: 'AMD',
      software: false,
      reason: 'major-performance-caveat',
    });
    expect(loseContext).toHaveBeenCalledOnce();
  });

  it('reports unsupported when neither strict nor probe contexts are available', () => {
    const canvas = createCanvas({
      strictContext: null,
      probeContext: null,
    });

    const result = selectWebGLContext(canvas);

    expect(result).toEqual({
      gl: null,
      diagnostics: {
        backend: 'canvas2d',
        renderer: null,
        vendor: null,
        software: false,
        reason: 'unsupported',
      },
    });
  });

  it('reports initialization-failed when strict context creation throws', () => {
    const canvas = createCanvas({
      strictContext: null,
      strictError: new Error('driver init failed'),
    });

    const result = selectWebGLContext(canvas);

    expect(result).toEqual({
      gl: null,
      diagnostics: {
        backend: 'canvas2d',
        renderer: null,
        vendor: null,
        software: false,
        reason: 'initialization-failed',
      },
    });
  });
});
