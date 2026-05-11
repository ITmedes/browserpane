import { afterEach, describe, expect, it, vi } from 'vitest';

import { SessionResizeRuntime } from '../session-resize-runtime.js';

type Size = {
  width: number;
  height: number;
};

function setRect(element: HTMLElement, size: Size): void {
  element.getBoundingClientRect = () => ({
    width: size.width,
    height: size.height,
    top: 0,
    left: 0,
    right: size.width,
    bottom: size.height,
    x: 0,
    y: 0,
    toJSON: () => {},
  });
}

function createRuntime(canvasSize: Size): {
  runtime: SessionResizeRuntime;
  canvas: HTMLCanvasElement;
  sendResizeRequest: ReturnType<typeof vi.fn>;
} {
  const container = document.createElement('div');
  const canvas = document.createElement('canvas');
  const sendResizeRequest = vi.fn();
  setRect(container, { width: 1200, height: 900 });
  setRect(canvas, canvasSize);
  const runtime = new SessionResizeRuntime({
    container,
    canvas,
    cursorEl: null,
    hiDpi: false,
    resizeObserver: { observe: vi.fn(), disconnect: vi.fn() },
    resizeRenderer: vi.fn(),
    markDisplayDirty: vi.fn(),
    sendResizeRequest,
    setRemoteSize: vi.fn(),
  });
  return { runtime, canvas, sendResizeRequest };
}

describe('SessionResizeRuntime canvas sizing', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('initializes from the rendered canvas size instead of the outer container', () => {
    const { runtime, canvas } = createRuntime({ width: 640, height: 480 });

    const size = runtime.initializeCanvasSize();

    expect(size).toEqual({ width: 640, height: 480 });
    expect(canvas.width).toBe(640);
    expect(canvas.height).toBe(480);
  });

  it('sends resize requests from the rendered canvas size when available', async () => {
    vi.useFakeTimers();
    const sendResizeRequest = vi.fn();
    const container = document.createElement('div');
    const canvas = document.createElement('canvas');
    setRect(container, { width: 1200, height: 900 });
    setRect(canvas, { width: 700, height: 500 });
    const runtime = new SessionResizeRuntime({
      container,
      canvas,
      cursorEl: null,
      hiDpi: false,
      resizeObserver: { observe: vi.fn(), disconnect: vi.fn() },
      resizeRenderer: vi.fn(),
      markDisplayDirty: vi.fn(),
      sendResizeRequest,
      setRemoteSize: vi.fn(),
    });

    runtime.handleResize(1200, 900);
    await vi.advanceTimersByTimeAsync(150);

    expect(sendResizeRequest).toHaveBeenCalledWith(700, 500);
  });
});
