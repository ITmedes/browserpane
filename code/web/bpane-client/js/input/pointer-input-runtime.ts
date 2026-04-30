import { createScrollState, normalizeScroll, type ScrollState } from '../input-map.js';

const INPUT_THROTTLE_MS = 16;

export interface PointerInputRuntimeInput {
  canvas: HTMLCanvasElement;
  drawCursor: (shape: null, x: number, y: number) => void;
  getRemoteDims: () => { width: number; height: number };
  sendMouseMove: (x: number, y: number) => void;
  sendMouseButton: (button: number, down: boolean, x: number, y: number) => void;
  sendScroll: (dx: number, dy: number) => void;
  now?: () => number;
}

export interface PointerInputBindingInput {
  signal: AbortSignal;
  focusKeyboardTarget: () => void;
}

export class PointerInputRuntime {
  private readonly canvas: HTMLCanvasElement;
  private readonly drawCursor: (shape: null, x: number, y: number) => void;
  private readonly getRemoteDims: () => { width: number; height: number };
  private readonly sendMouseMoveFn: (x: number, y: number) => void;
  private readonly sendMouseButtonFn: (button: number, down: boolean, x: number, y: number) => void;
  private readonly sendScrollFn: (dx: number, dy: number) => void;
  private readonly now: () => number;

  private lastMouseSendTime = 0;
  private scrollState: ScrollState = createScrollState();

  constructor(input: PointerInputRuntimeInput) {
    this.canvas = input.canvas;
    this.drawCursor = input.drawCursor;
    this.getRemoteDims = input.getRemoteDims;
    this.sendMouseMoveFn = input.sendMouseMove;
    this.sendMouseButtonFn = input.sendMouseButton;
    this.sendScrollFn = input.sendScroll;
    this.now = input.now ?? (() => performance.now());
  }

  bind(input: PointerInputBindingInput): void {
    this.canvas.addEventListener('pointermove', (event: PointerEvent) => {
      const now = this.now();
      if (now - this.lastMouseSendTime < INPUT_THROTTLE_MS) {
        return;
      }
      this.lastMouseSendTime = now;

      const { x, y } = this.resolveCanvasPoint(event.clientX, event.clientY);
      this.drawCursor(null, x, y);
      this.sendMouseMoveFn(x, y);
    }, { signal: input.signal });

    this.canvas.addEventListener('pointerdown', (event: PointerEvent) => {
      event.preventDefault();
      const { x, y } = this.resolveCanvasPoint(event.clientX, event.clientY);
      this.sendMouseButtonFn(event.button, true, x, y);
    }, { signal: input.signal });

    this.canvas.addEventListener('pointerup', (event: PointerEvent) => {
      event.preventDefault();
      const { x, y } = this.resolveCanvasPoint(event.clientX, event.clientY);
      this.sendMouseButtonFn(event.button, false, x, y);
    }, { signal: input.signal });

    this.canvas.addEventListener('wheel', (event: WheelEvent) => {
      event.preventDefault();
      const { dx, dy } = normalizeScroll(
        event.deltaX,
        event.deltaY,
        event.deltaMode,
        this.scrollState,
      );
      if (dx || dy) {
        const { x, y } = this.resolveCanvasPoint(event.clientX, event.clientY);
        this.drawCursor(null, x, y);
        this.sendMouseMoveFn(x, y);
        this.sendScrollFn(dx, dy);
      }
    }, { passive: false, signal: input.signal });

    this.canvas.addEventListener('contextmenu', (event) => {
      event.preventDefault();
    }, { signal: input.signal });

    this.canvas.addEventListener('click', () => {
      input.focusKeyboardTarget();
    }, { signal: input.signal });
  }

  reset(): void {
    this.lastMouseSendTime = 0;
    this.scrollState = createScrollState();
  }

  private resolveCanvasPoint(clientX: number, clientY: number): { x: number; y: number } {
    const rect = this.canvas.getBoundingClientRect();
    const { width: targetWidth, height: targetHeight } = this.getRemoteDims();
    const scaleX = targetWidth / rect.width;
    const scaleY = targetHeight / rect.height;
    return {
      x: Math.round((clientX - rect.left) * scaleX),
      y: Math.round((clientY - rect.top) * scaleY),
    };
  }
}
