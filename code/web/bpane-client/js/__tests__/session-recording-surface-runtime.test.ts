import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { SessionRecordingSurfaceRuntime } from '../session-recording-surface-runtime.js';

function createCanvas(width: number, height: number): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  return canvas;
}

describe('SessionRecordingSurfaceRuntime', () => {
  let contexts: WeakMap<HTMLCanvasElement, CanvasRenderingContext2D>;
  let captureStreamSpy: ReturnType<typeof vi.fn>;
  let rafCallback: FrameRequestCallback | null;
  let cancelAnimationFrameSpy: ReturnType<typeof vi.fn>;
  let captureTrackStopSpy: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    contexts = new WeakMap();
    captureTrackStopSpy = vi.fn();
    captureStreamSpy = vi.fn(() => ({
      getTracks: () => [{ stop: captureTrackStopSpy }],
    }) as unknown as MediaStream);
    rafCallback = null;
    cancelAnimationFrameSpy = vi.fn();

    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (
      this: HTMLCanvasElement,
      contextId: string,
    ) {
      if (contextId !== '2d') return null;
      let context = contexts.get(this);
      if (!context) {
        context = {
          clearRect: vi.fn(),
          drawImage: vi.fn(),
          canvas: this,
        } as unknown as CanvasRenderingContext2D;
        contexts.set(this, context);
      }
      return context;
    });
    Object.defineProperty(HTMLCanvasElement.prototype, 'captureStream', {
      configurable: true,
      writable: true,
      value: captureStreamSpy,
    });
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      rafCallback = callback;
      return 17;
    }));
    vi.stubGlobal('cancelAnimationFrame', cancelAnimationFrameSpy);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('creates a capture stream from a dedicated recording surface and composites main output plus cursor overlay', () => {
    const sourceCanvas = createCanvas(1280, 720);
    const cursorCanvas = createCanvas(1280, 720);
    const runtime = new SessionRecordingSurfaceRuntime({
      sourceCanvas,
      cursorCanvas,
    });

    const stream = runtime.start(30);

    expect(stream).toBeDefined();
    expect(captureStreamSpy).toHaveBeenCalledOnce();
    expect(captureStreamSpy).toHaveBeenCalledWith(30);
    expect(rafCallback).toBeTypeOf('function');

    rafCallback?.(0);

    const recordingCanvas = captureStreamSpy.mock.instances[0] as HTMLCanvasElement;
    const recordingContext = contexts.get(recordingCanvas)! as any;
    expect(recordingCanvas.width).toBe(1280);
    expect(recordingCanvas.height).toBe(720);
    expect(recordingContext.clearRect).toHaveBeenCalledWith(0, 0, 1280, 720);
    expect(recordingContext.drawImage).toHaveBeenNthCalledWith(1, sourceCanvas, 0, 0, 1280, 720);
    expect(recordingContext.drawImage).toHaveBeenNthCalledWith(2, cursorCanvas, 0, 0, 1280, 720);
  });

  it('cancels the mirror loop when stopped', () => {
    const runtime = new SessionRecordingSurfaceRuntime({
      sourceCanvas: createCanvas(800, 600),
      cursorCanvas: createCanvas(800, 600),
    });

    runtime.start(24);
    runtime.stop();

    expect(cancelAnimationFrameSpy).toHaveBeenCalledWith(17);
  });

  it('stops the capture stream tracks when stopped', () => {
    const runtime = new SessionRecordingSurfaceRuntime({
      sourceCanvas: createCanvas(800, 600),
      cursorCanvas: createCanvas(800, 600),
    });

    runtime.start(24);
    runtime.stop();

    expect(captureTrackStopSpy).toHaveBeenCalledOnce();
  });
});
