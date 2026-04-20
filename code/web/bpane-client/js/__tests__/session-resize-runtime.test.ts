import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { SessionResizeRuntime } from '../session-resize-runtime.js';

type RectState = {
  width: number;
  height: number;
};

function createContainer(initialWidth = 800, initialHeight = 600): {
  container: HTMLDivElement;
  setRect: (width: number, height: number) => void;
} {
  const container = document.createElement('div');
  let rect: RectState = {
    width: initialWidth,
    height: initialHeight,
  };
  container.getBoundingClientRect = () => ({
    width: rect.width,
    height: rect.height,
    top: 0,
    left: 0,
    right: rect.width,
    bottom: rect.height,
    x: 0,
    y: 0,
    toJSON: () => {},
  });
  return {
    container,
    setRect: (width: number, height: number) => {
      rect = { width, height };
    },
  };
}

function createRuntime(overrides: Partial<ConstructorParameters<typeof SessionResizeRuntime>[0]> = {}) {
  const { container, setRect } = createContainer();
  const canvas = document.createElement('canvas');
  canvas.width = 0;
  canvas.height = 0;
  const cursorEl = document.createElement('canvas');
  cursorEl.width = 0;
  cursorEl.height = 0;
  const resizeObserver = {
    observe: vi.fn(),
    disconnect: vi.fn(),
  };
  const resizeRenderer = vi.fn();
  const markDisplayDirty = vi.fn();
  const sendResizeRequest = vi.fn();
  const setRemoteSize = vi.fn();
  const onResolutionChange = vi.fn();

  const runtime = new SessionResizeRuntime({
    container,
    canvas,
    cursorEl,
    hiDpi: false,
    resizeObserver,
    resizeRenderer,
    markDisplayDirty,
    sendResizeRequest,
    setRemoteSize,
    onResolutionChange,
    ...overrides,
  });

  return {
    runtime,
    container,
    canvas,
    cursorEl,
    resizeObserver,
    resizeRenderer,
    markDisplayDirty,
    sendResizeRequest,
    setRemoteSize,
    onResolutionChange,
    setRect,
  };
}

describe('SessionResizeRuntime', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'devicePixelRatio', {
      configurable: true,
      value: 1,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('debounces container resize and applies scaled dimensions', async () => {
    vi.useFakeTimers();
    Object.defineProperty(window, 'devicePixelRatio', {
      configurable: true,
      value: 2,
    });
    const {
      runtime,
      canvas,
      cursorEl,
      resizeRenderer,
      markDisplayDirty,
      sendResizeRequest,
    } = createRuntime({
      hiDpi: true,
    });

    runtime.handleResize(640, 480);
    runtime.handleResize(900, 700);

    expect(sendResizeRequest).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(149);
    expect(sendResizeRequest).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);

    expect(canvas.width).toBe(1800);
    expect(canvas.height).toBe(1400);
    expect(cursorEl.width).toBe(1800);
    expect(cursorEl.height).toBe(1400);
    expect(resizeRenderer).toHaveBeenCalledWith(1800, 1400);
    expect(markDisplayDirty).toHaveBeenCalledOnce();
    expect(sendResizeRequest).toHaveBeenCalledWith(1800, 1400);
  });

  it('locks remote resolution while preserving the local container size and ignores later resize events', async () => {
    vi.useFakeTimers();
    const {
      runtime,
      container,
      canvas,
      cursorEl,
      resizeObserver,
      resizeRenderer,
      markDisplayDirty,
      sendResizeRequest,
      setRemoteSize,
      onResolutionChange,
    } = createRuntime();

    runtime.applyClientAccessState(0x02, 1280, 720);

    expect(runtime.isResolutionLocked()).toBe(true);
    expect(resizeObserver.disconnect).toHaveBeenCalledOnce();
    expect(canvas.width).toBe(1280);
    expect(canvas.height).toBe(720);
    expect(cursorEl.width).toBe(1280);
    expect(cursorEl.height).toBe(720);
    expect(container.style.width).toBe('');
    expect(container.style.height).toBe('');
    expect(container.style.maxWidth).toBe('');
    expect(container.style.maxHeight).toBe('');
    expect(setRemoteSize).toHaveBeenCalledWith(1280, 720);
    expect(onResolutionChange).toHaveBeenCalledWith(1280, 720);
    expect(resizeRenderer).toHaveBeenCalledWith(1280, 720);

    runtime.handleResize(900, 700);
    await vi.advanceTimersByTimeAsync(200);

    expect(sendResizeRequest).not.toHaveBeenCalled();
    expect(markDisplayDirty).not.toHaveBeenCalled();
  });

  it('unlocks resolution and resumes container-driven sizing', () => {
    const {
      runtime,
      container,
      canvas,
      cursorEl,
      resizeObserver,
      resizeRenderer,
      markDisplayDirty,
      sendResizeRequest,
      setRect,
    } = createRuntime();

    runtime.applyClientAccessState(0x02, 1280, 720);
    resizeRenderer.mockClear();
    setRect(500, 400);

    runtime.applyClientAccessState(0x00, 0, 0);

    expect(runtime.isResolutionLocked()).toBe(false);
    expect(resizeObserver.observe).toHaveBeenCalledWith(container);
    expect(canvas.width).toBe(500);
    expect(canvas.height).toBe(400);
    expect(cursorEl.width).toBe(500);
    expect(cursorEl.height).toBe(400);
    expect(resizeRenderer).toHaveBeenCalledWith(500, 400);
    expect(markDisplayDirty).toHaveBeenCalledOnce();
    expect(sendResizeRequest).toHaveBeenCalledWith(500, 400);
  });

  it('tracks resize lock without dimensions until the gateway provides them', () => {
    const {
      runtime,
      canvas,
      resizeObserver,
      resizeRenderer,
      setRemoteSize,
      onResolutionChange,
    } = createRuntime();

    runtime.applyClientAccessState(0x02, 0, 0);

    expect(runtime.isResolutionLocked()).toBe(true);
    expect(canvas.width).toBe(0);
    expect(canvas.height).toBe(0);
    expect(resizeObserver.disconnect).not.toHaveBeenCalled();
    expect(resizeRenderer).not.toHaveBeenCalled();
    expect(setRemoteSize).not.toHaveBeenCalled();
    expect(onResolutionChange).not.toHaveBeenCalled();
  });

  it('clears pending resize work on destroy', async () => {
    vi.useFakeTimers();
    const {
      runtime,
      resizeObserver,
      sendResizeRequest,
    } = createRuntime();

    runtime.handleResize(700, 500);
    runtime.destroy();
    await vi.advanceTimersByTimeAsync(200);

    expect(resizeObserver.disconnect).toHaveBeenCalledOnce();
    expect(sendResizeRequest).not.toHaveBeenCalled();
  });
});
