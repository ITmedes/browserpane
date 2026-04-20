import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SessionSurfaceRuntime } from '../session-surface-runtime.js';
import { installCanvasGetContextMock } from './canvas-test-helpers.js';

function createContainer(): HTMLDivElement {
  const container = document.createElement('div');
  Object.defineProperty(container, 'clientWidth', { value: 800 });
  Object.defineProperty(container, 'clientHeight', { value: 600 });
  container.getBoundingClientRect = () => ({
    width: 800,
    height: 600,
    top: 0,
    left: 0,
    right: 800,
    bottom: 600,
    x: 0,
    y: 0,
    toJSON: () => {},
  });
  return container;
}

function createTileCompositor() {
  return {
    setContext: vi.fn(),
    setWebGLRenderer: vi.fn(),
    setCacheMissHandler: vi.fn(),
    getGridConfig: vi.fn(() => null),
    getVideoRegion: vi.fn(() => null),
  };
}

describe('SessionSurfaceRuntime', () => {
  let observe: ReturnType<typeof vi.fn>;
  let disconnect: ReturnType<typeof vi.fn>;
  let canvasGetContextSpy: ReturnType<typeof installCanvasGetContextMock>;

  beforeEach(() => {
    observe = vi.fn();
    disconnect = vi.fn();
    canvasGetContextSpy = installCanvasGetContextMock();
    (globalThis as any).ResizeObserver = vi.fn(() => ({
      observe,
      disconnect,
    }));
    (globalThis as any).requestAnimationFrame = vi.fn(() => 1);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('mounts the canvas surface, cursor overlay, and forced Canvas2D renderer state', () => {
    const container = createContainer();
    const tileCompositor = createTileCompositor();

    const runtime = new SessionSurfaceRuntime({
      container,
      tileCompositor,
      hiDpi: false,
      renderBackend: 'canvas2d',
      onTileCacheMiss: vi.fn(),
      sendResizeRequest: vi.fn(),
      setRemoteSize: vi.fn(),
    });

    runtime.start();

    const canvases = container.querySelectorAll('canvas');
    expect(canvases).toHaveLength(2);
    expect(runtime.getCanvas()).toBe(canvases[0]);
    expect(canvases[0].style.width).toBe('100%');
    expect(canvases[0].style.height).toBe('100%');
    expect(canvases[0].style.cursor).toBe('none');
    expect(canvases[1].style.position).toBe('absolute');
    expect(container.style.position).toBe('relative');
    expect(runtime.getCanvas().width).toBe(800);
    expect(runtime.getCanvas().height).toBe(600);
    expect(tileCompositor.setContext).toHaveBeenCalledOnce();
    expect(tileCompositor.setCacheMissHandler).toHaveBeenCalledWith(expect.any(Function));
    expect(observe).toHaveBeenCalledWith(container);
    expect(runtime.getRenderDiagnostics()).toEqual({
      backend: 'canvas2d',
      renderer: null,
      vendor: null,
      software: false,
      reason: 'forced-canvas2d',
    });
    expect(canvasGetContextSpy.mock.calls.some(([contextId]) => contextId === 'webgl2')).toBe(false);
  });

  it('tears down observers, compositor hooks, and mounted canvases on destroy', () => {
    const container = createContainer();
    const tileCompositor = createTileCompositor();
    const runtime = new SessionSurfaceRuntime({
      container,
      tileCompositor,
      hiDpi: false,
      renderBackend: 'canvas2d',
      onTileCacheMiss: vi.fn(),
      sendResizeRequest: vi.fn(),
      setRemoteSize: vi.fn(),
    });

    runtime.destroy();

    expect(disconnect).toHaveBeenCalledOnce();
    expect(container.querySelectorAll('canvas')).toHaveLength(0);
    expect(tileCompositor.setCacheMissHandler).toHaveBeenLastCalledWith(null);
    expect(tileCompositor.setWebGLRenderer).toHaveBeenCalledWith(null);
  });
});
