import { beforeEach, describe, expect, it, vi } from 'vitest';
import { PointerInputRuntime } from '../input/pointer-input-runtime.js';

function dispatchPointer(
  target: HTMLElement,
  type: 'pointermove' | 'pointerdown' | 'pointerup',
  init: {
    clientX: number;
    clientY: number;
    button?: number;
  },
): Event {
  const event = new Event(type, {
    bubbles: true,
    cancelable: true,
  });
  Object.defineProperties(event, {
    clientX: { configurable: true, value: init.clientX },
    clientY: { configurable: true, value: init.clientY },
    button: { configurable: true, value: init.button ?? 0 },
  });
  target.dispatchEvent(event);
  return event;
}

function dispatchWheel(
  target: HTMLElement,
  init: {
    clientX?: number;
    clientY?: number;
    deltaX?: number;
    deltaY: number;
    deltaMode?: number;
  },
): Event {
  const event = new Event('wheel', {
    bubbles: true,
    cancelable: true,
  });
  Object.defineProperties(event, {
    clientX: { configurable: true, value: init.clientX ?? 0 },
    clientY: { configurable: true, value: init.clientY ?? 0 },
    deltaX: { configurable: true, value: init.deltaX ?? 0 },
    deltaY: { configurable: true, value: init.deltaY },
    deltaMode: { configurable: true, value: init.deltaMode ?? 0 },
  });
  target.dispatchEvent(event);
  return event;
}

function setCanvasRect(canvas: HTMLCanvasElement, rect: {
  left: number;
  top: number;
  width: number;
  height: number;
}): void {
  Object.defineProperty(canvas, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({
      ...rect,
      right: rect.left + rect.width,
      bottom: rect.top + rect.height,
      x: rect.left,
      y: rect.top,
      toJSON: () => ({}),
    }),
  });
}

beforeEach(() => {
  document.body.innerHTML = '';
});

describe('PointerInputRuntime', () => {
  it('scales and throttles pointer move events', () => {
    const canvas = document.createElement('canvas');
    const sendMouseMove = vi.fn();
    const drawCursor = vi.fn();
    let now = 100;
    setCanvasRect(canvas, {
      left: 10,
      top: 20,
      width: 400,
      height: 300,
    });

    const runtime = new PointerInputRuntime({
      canvas,
      drawCursor,
      getRemoteDims: () => ({ width: 800, height: 600 }),
      sendMouseMove,
      sendMouseButton: vi.fn(),
      sendScroll: vi.fn(),
      now: () => now,
    });
    runtime.bind({
      signal: new AbortController().signal,
      focusKeyboardTarget: vi.fn(),
    });

    dispatchPointer(canvas, 'pointermove', { clientX: 110, clientY: 170 });
    now = 110;
    dispatchPointer(canvas, 'pointermove', { clientX: 120, clientY: 180 });
    now = 130;
    dispatchPointer(canvas, 'pointermove', { clientX: 210, clientY: 220 });

    expect(sendMouseMove.mock.calls).toEqual([
      [200, 300],
      [400, 400],
    ]);
    expect(drawCursor.mock.calls).toEqual([
      [null, 200, 300],
      [null, 400, 400],
    ]);
  });

  it('sends pointer buttons, accumulates wheel deltas, and focuses the keyboard target on click', () => {
    const canvas = document.createElement('canvas');
    const sendMouseButton = vi.fn();
    const sendMouseMove = vi.fn();
    const sendScroll = vi.fn();
    const focusKeyboardTarget = vi.fn();
    setCanvasRect(canvas, {
      left: 50,
      top: 100,
      width: 200,
      height: 150,
    });

    const runtime = new PointerInputRuntime({
      canvas,
      drawCursor: vi.fn(),
      getRemoteDims: () => ({ width: 800, height: 600 }),
      sendMouseMove,
      sendMouseButton,
      sendScroll,
      now: () => 100,
    });
    runtime.bind({
      signal: new AbortController().signal,
      focusKeyboardTarget,
    });

    const pointerDown = dispatchPointer(canvas, 'pointerdown', {
      clientX: 150,
      clientY: 175,
      button: 2,
    });
    const pointerUp = dispatchPointer(canvas, 'pointerup', {
      clientX: 150,
      clientY: 175,
      button: 2,
    });
    const firstWheel = dispatchWheel(canvas, { clientX: 150, clientY: 175, deltaY: 30 });
    const secondWheel = dispatchWheel(canvas, { clientX: 150, clientY: 175, deltaY: 30 });
    const contextMenu = new Event('contextmenu', {
      bubbles: true,
      cancelable: true,
    });
    canvas.dispatchEvent(contextMenu);
    canvas.dispatchEvent(new MouseEvent('click', {
      bubbles: true,
      cancelable: true,
    }));

    expect(pointerDown.defaultPrevented).toBe(true);
    expect(pointerUp.defaultPrevented).toBe(true);
    expect(firstWheel.defaultPrevented).toBe(true);
    expect(secondWheel.defaultPrevented).toBe(true);
    expect(contextMenu.defaultPrevented).toBe(true);
    expect(sendMouseButton.mock.calls).toEqual([
      [2, true, 400, 300],
      [2, false, 400, 300],
    ]);
    expect(sendMouseMove.mock.calls).toEqual([
      [400, 300],
    ]);
    expect(sendScroll.mock.calls).toEqual([
      [0, -1],
    ]);
    expect(focusKeyboardTarget).toHaveBeenCalledTimes(1);
  });

  it('resets scroll accumulation and pointer throttle state', () => {
    const canvas = document.createElement('canvas');
    const sendMouseMove = vi.fn();
    const sendScroll = vi.fn();
    let now = 100;
    setCanvasRect(canvas, {
      left: 0,
      top: 0,
      width: 100,
      height: 100,
    });

    const runtime = new PointerInputRuntime({
      canvas,
      drawCursor: vi.fn(),
      getRemoteDims: () => ({ width: 100, height: 100 }),
      sendMouseMove,
      sendMouseButton: vi.fn(),
      sendScroll,
      now: () => now,
    });
    runtime.bind({
      signal: new AbortController().signal,
      focusKeyboardTarget: vi.fn(),
    });

    dispatchWheel(canvas, { clientX: 5, clientY: 5, deltaY: 30 });
    dispatchPointer(canvas, 'pointermove', { clientX: 10, clientY: 10 });
    runtime.reset();
    now = 110;
    dispatchPointer(canvas, 'pointermove', { clientX: 20, clientY: 20 });
    dispatchWheel(canvas, { clientX: 30, clientY: 40, deltaY: 30 });
    dispatchWheel(canvas, { clientX: 30, clientY: 40, deltaY: 30 });

    expect(sendMouseMove.mock.calls).toEqual([
      [10, 10],
      [20, 20],
      [30, 40],
    ]);
    expect(sendScroll.mock.calls).toEqual([
      [0, -1],
    ]);
  });
});
