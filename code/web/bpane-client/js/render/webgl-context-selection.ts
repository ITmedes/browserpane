export type RenderBackend = 'webgl2' | 'canvas2d';

export type RenderSelectionReason =
  | 'hardware-accelerated'
  | 'unsupported'
  | 'major-performance-caveat'
  | 'software-renderer'
  | 'initialization-failed';

export interface WebGLContextInfo {
  renderer: string | null;
  vendor: string | null;
  software: boolean;
}

export interface WebGLRendererDiagnostics extends WebGLContextInfo {
  backend: RenderBackend;
  reason: RenderSelectionReason;
}

export interface WebGLContextSelectionResult {
  gl: WebGL2RenderingContext | null;
  diagnostics: WebGLRendererDiagnostics;
}

const SOFTWARE_RENDERER_PATTERNS = [
  /swiftshader/i,
  /\bllvmpipe\b/i,
  /\blavapipe\b/i,
  /\bsoftpipe\b/i,
  /software rasterizer/i,
  /software renderer/i,
];

function createFallbackDiagnostics(reason: RenderSelectionReason): WebGLRendererDiagnostics {
  return {
    backend: 'canvas2d',
    renderer: null,
    vendor: null,
    software: false,
    reason,
  };
}

function isSoftwareRenderer(renderer: string | null, vendor: string | null): boolean {
  if (renderer && SOFTWARE_RENDERER_PATTERNS.some((pattern) => pattern.test(renderer))) {
    return true;
  }
  return vendor !== null && /swiftshader/i.test(vendor);
}

export function detectContextInfo(gl: WebGL2RenderingContext): WebGLContextInfo {
  let renderer: string | null = null;
  let vendor: string | null = null;

  const maskedRenderer = gl.getParameter(gl.RENDERER);
  if (typeof maskedRenderer === 'string' && maskedRenderer.length > 0) {
    renderer = maskedRenderer;
  }

  const maskedVendor = gl.getParameter(gl.VENDOR);
  if (typeof maskedVendor === 'string' && maskedVendor.length > 0) {
    vendor = maskedVendor;
  }

  const debugInfo = gl.getExtension('WEBGL_debug_renderer_info');
  if (debugInfo) {
    const unmaskedRenderer = gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL);
    if (typeof unmaskedRenderer === 'string' && unmaskedRenderer.length > 0) {
      renderer = unmaskedRenderer;
    }

    const unmaskedVendor = gl.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL);
    if (typeof unmaskedVendor === 'string' && unmaskedVendor.length > 0) {
      vendor = unmaskedVendor;
    }
  }

  return {
    renderer,
    vendor,
    software: isSoftwareRenderer(renderer, vendor),
  };
}

function loseContext(gl: WebGL2RenderingContext): void {
  const extension = gl.getExtension('WEBGL_lose_context');
  extension?.loseContext();
}

function probeCanvas(source: HTMLCanvasElement): HTMLCanvasElement | null {
  const doc = source.ownerDocument ?? (typeof document !== 'undefined' ? document : null);
  return doc ? doc.createElement('canvas') : null;
}

export function selectWebGLContext(canvas: HTMLCanvasElement): WebGLContextSelectionResult {
  const baseAttrs: WebGLContextAttributes = {
    alpha: false,
    antialias: false,
    preserveDrawingBuffer: true,
    desynchronized: true,
    powerPreference: 'high-performance',
  };

  try {
    const gl = canvas.getContext('webgl2', {
      ...baseAttrs,
      failIfMajorPerformanceCaveat: true,
    });
    if (!gl) {
      const probe = probeCanvas(canvas);
      const probeGl = probe?.getContext('webgl2', baseAttrs) ?? null;
      if (!probeGl) {
        return {
          gl: null,
          diagnostics: createFallbackDiagnostics('unsupported'),
        };
      }

      const probeInfo = detectContextInfo(probeGl);
      loseContext(probeGl);
      return {
        gl: null,
        diagnostics: {
          backend: 'canvas2d',
          renderer: probeInfo.renderer,
          vendor: probeInfo.vendor,
          software: probeInfo.software,
          reason: probeInfo.software ? 'software-renderer' : 'major-performance-caveat',
        },
      };
    }

    const info = detectContextInfo(gl);
    if (info.software) {
      loseContext(gl);
      return {
        gl: null,
        diagnostics: {
          backend: 'canvas2d',
          renderer: info.renderer,
          vendor: info.vendor,
          software: true,
          reason: 'software-renderer',
        },
      };
    }

    return {
      gl,
      diagnostics: {
        backend: 'webgl2',
        renderer: info.renderer,
        vendor: info.vendor,
        software: false,
        reason: 'hardware-accelerated',
      },
    };
  } catch {
    return {
      gl: null,
      diagnostics: createFallbackDiagnostics('initialization-failed'),
    };
  }
}
