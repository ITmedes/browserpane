const RESIZE_DEBOUNCE_MS = 150;

export interface SessionResizeRuntimeInput {
  container: HTMLElement;
  canvas: HTMLCanvasElement;
  cursorEl: HTMLCanvasElement | null;
  hiDpi: boolean;
  resizeObserver: Pick<ResizeObserver, 'observe' | 'disconnect'>;
  resizeRenderer: (width: number, height: number) => void;
  markDisplayDirty: () => void;
  sendResizeRequest: (width: number, height: number) => void;
  setRemoteSize: (width: number, height: number) => void;
  onResolutionChange?: (width: number, height: number) => void;
  resizeDebounceMs?: number;
  setTimeoutFn?: Window['setTimeout'];
  clearTimeoutFn?: Window['clearTimeout'];
  getDevicePixelRatio?: () => number;
}

export class SessionResizeRuntime {
  private readonly container: HTMLElement;
  private readonly canvas: HTMLCanvasElement;
  private readonly cursorEl: HTMLCanvasElement | null;
  private readonly hiDpi: boolean;
  private readonly resizeObserver: Pick<ResizeObserver, 'observe' | 'disconnect'>;
  private readonly resizeRenderer: (width: number, height: number) => void;
  private readonly markDisplayDirty: () => void;
  private readonly sendResizeRequest: (width: number, height: number) => void;
  private readonly setRemoteSize: (width: number, height: number) => void;
  private readonly onResolutionChange?: (width: number, height: number) => void;
  private readonly resizeDebounceMs: number;
  private readonly setTimeoutFn: Window['setTimeout'];
  private readonly clearTimeoutFn: Window['clearTimeout'];
  private readonly getDevicePixelRatio: () => number;

  private resizeTimeout: number | null = null;
  private resolutionLocked = false;

  constructor(input: SessionResizeRuntimeInput) {
    this.container = input.container;
    this.canvas = input.canvas;
    this.cursorEl = input.cursorEl;
    this.hiDpi = input.hiDpi;
    this.resizeObserver = input.resizeObserver;
    this.resizeRenderer = input.resizeRenderer;
    this.markDisplayDirty = input.markDisplayDirty;
    this.sendResizeRequest = input.sendResizeRequest;
    this.setRemoteSize = input.setRemoteSize;
    this.onResolutionChange = input.onResolutionChange;
    this.resizeDebounceMs = input.resizeDebounceMs ?? RESIZE_DEBOUNCE_MS;
    this.setTimeoutFn = input.setTimeoutFn ?? window.setTimeout.bind(window);
    this.clearTimeoutFn = input.clearTimeoutFn ?? window.clearTimeout.bind(window);
    this.getDevicePixelRatio = input.getDevicePixelRatio ?? (() => window.devicePixelRatio || 1);
  }

  initializeCanvasSize(): { width: number; height: number } {
    const dims = this.getContainerResizeDims();
    this.applyCanvasSize(dims.width, dims.height);
    return dims;
  }

  getContainerResizeDims(fallback?: { width: number; height: number }): { width: number; height: number } {
    const rect = this.getResizeSource(fallback);
    return this.scaledDims(rect.width, rect.height);
  }

  handleResize(width: number, height: number): void {
    if (this.resolutionLocked) {
      return;
    }

    if (this.resizeTimeout !== null) {
      this.clearTimeoutFn(this.resizeTimeout);
    }

    this.resizeTimeout = this.setTimeoutFn(() => {
      this.resizeTimeout = null;
      const dims = this.getContainerResizeDims({ width, height });
      this.applyCanvasSize(dims.width, dims.height);
      this.markDisplayDirty();
      this.sendResizeRequest(dims.width, dims.height);
    }, this.resizeDebounceMs);
  }

  applyClientAccessState(flags: number, width: number, height: number): void {
    const resizeLocked = (flags & 0x02) !== 0;

    if (resizeLocked) {
      if (width > 0 && height > 0) {
        this.setResolutionLock(width, height);
      } else {
        this.resolutionLocked = true;
      }
      return;
    }

    this.unlockResolution();
  }

  isResolutionLocked(): boolean {
    return this.resolutionLocked;
  }

  destroy(): void {
    if (this.resizeTimeout !== null) {
      this.clearTimeoutFn(this.resizeTimeout);
      this.resizeTimeout = null;
    }
    this.resizeObserver.disconnect();
  }

  private setResolutionLock(width: number, height: number): void {
    this.resolutionLocked = true;
    this.setRemoteSize(width, height);
    this.resizeObserver.disconnect();

    if (this.resizeTimeout !== null) {
      this.clearTimeoutFn(this.resizeTimeout);
      this.resizeTimeout = null;
    }

    this.applyCanvasSize(width, height);

    const scale = this.computeScale();
    const cssW = Math.ceil(width / scale);
    const cssH = Math.ceil(height / scale);
    const rect = this.container.getBoundingClientRect();

    if (rect.width >= cssW && rect.height >= cssH) {
      this.container.style.flex = 'none';
      this.container.style.width = `${cssW}px`;
      this.container.style.height = `${cssH}px`;
      this.container.style.resize = 'none';
      this.container.style.maxWidth = `${cssW}px`;
      this.container.style.maxHeight = `${cssH}px`;
      console.log(`[bpane] resolution locked to ${width}x${height} (container fixed to ${cssW}x${cssH}px)`);
    } else {
      console.log(`[bpane] resolution locked to ${width}x${height} (scaled to local container)`);
    }

    this.onResolutionChange?.(width, height);
  }

  private unlockResolution(): void {
    if (!this.resolutionLocked) {
      return;
    }

    this.resolutionLocked = false;
    this.container.style.flex = '';
    this.container.style.width = '';
    this.container.style.height = '';
    this.container.style.resize = '';
    this.container.style.maxWidth = '';
    this.container.style.maxHeight = '';
    this.resizeObserver.observe(this.container);

    const dims = this.getContainerResizeDims();
    this.applyCanvasSize(dims.width, dims.height);
    this.markDisplayDirty();
    this.sendResizeRequest(dims.width, dims.height);
  }

  private applyCanvasSize(width: number, height: number): void {
    this.canvas.width = width;
    this.canvas.height = height;
    this.resizeRenderer(width, height);
    if (this.cursorEl) {
      this.cursorEl.width = width;
      this.cursorEl.height = height;
    }
  }

  private computeScale(): number {
    if (!this.hiDpi) {
      return 1;
    }
    return Math.max(1, Math.min(3, this.getDevicePixelRatio()));
  }

  private getResizeSource(fallback?: { width: number; height: number }): { width: number; height: number } {
    const canvasRect = this.canvas.getBoundingClientRect();
    if (canvasRect.width > 0 && canvasRect.height > 0) return canvasRect;
    if (fallback && fallback.width > 0 && fallback.height > 0) return fallback;
    return this.container.getBoundingClientRect();
  }

  private scaledDims(width: number, height: number): { width: number; height: number } {
    const scale = this.computeScale();
    return {
      width: Math.floor(width * scale),
      height: Math.floor(height * scale),
    };
  }
}
