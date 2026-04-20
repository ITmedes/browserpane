import type { TileInfo } from './nal.js';
import type { CacheMissEvent } from './render/tile-draw-runtime.js';
import { SessionCursorRuntime } from './session-cursor-runtime.js';
import { SessionResizeRuntime } from './session-resize-runtime.js';
import { SessionVideoDisplayRuntime } from './session-video-display-runtime.js';
import {
  WebGLTileRenderer,
  type WebGLRendererCreationResult,
  type WebGLRendererDiagnostics,
} from './webgl-compositor.js';

export type SessionSurfaceRenderBackendPreference = 'auto' | 'canvas2d' | 'webgl2';

interface SessionSurfaceTileCompositor {
  setContext(ctx: CanvasRenderingContext2D): void;
  setWebGLRenderer(renderer: WebGLTileRenderer | null): void;
  setCacheMissHandler(handler: ((event: CacheMissEvent) => void) | null): void;
  getGridConfig(): { tileSize: number; cols: number; rows: number; screenW: number; screenH: number } | null;
  getVideoRegion(): { x: number; y: number; w: number; h: number } | null;
}

export interface SessionSurfaceRuntimeInput {
  container: HTMLElement;
  tileCompositor: SessionSurfaceTileCompositor;
  hiDpi: boolean;
  renderBackend?: SessionSurfaceRenderBackendPreference;
  onTileCacheMiss: (event: CacheMissEvent) => void;
  sendResizeRequest: (width: number, height: number) => void;
  setRemoteSize: (width: number, height: number) => void;
  onResolutionChange?: (width: number, height: number) => void;
  createResizeObserver?: (callback: ResizeObserverCallback) => Pick<ResizeObserver, 'observe' | 'disconnect'>;
  createWebGLRenderer?: (canvas: HTMLCanvasElement) => WebGLRendererCreationResult;
}

export class SessionSurfaceRuntime {
  private readonly container: HTMLElement;
  private readonly tileCompositor: SessionSurfaceTileCompositor;
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D | null;
  private cursorEl: HTMLCanvasElement | null;
  private readonly cursorRuntime: SessionCursorRuntime;
  private readonly resizeRuntime: SessionResizeRuntime;
  private readonly videoDisplayRuntime: SessionVideoDisplayRuntime;
  private glRenderer: WebGLTileRenderer | null = null;
  private renderDiagnostics: WebGLRendererDiagnostics = {
    backend: 'canvas2d',
    renderer: null,
    vendor: null,
    software: false,
    reason: 'unsupported',
  };

  constructor(input: SessionSurfaceRuntimeInput) {
    this.container = input.container;
    this.tileCompositor = input.tileCompositor;

    this.canvas = document.createElement('canvas');
    this.canvas.style.width = '100%';
    this.canvas.style.height = '100%';
    this.canvas.style.display = 'block';
    this.canvas.style.cursor = 'none';
    this.canvas.tabIndex = 0;
    this.container.appendChild(this.canvas);

    this.cursorEl = document.createElement('canvas');
    this.cursorEl.style.position = 'absolute';
    this.cursorEl.style.pointerEvents = 'none';
    this.cursorEl.style.top = '0';
    this.cursorEl.style.left = '0';
    this.cursorEl.style.width = '100%';
    this.cursorEl.style.height = '100%';
    this.cursorEl.width = Math.max(64, Math.floor(this.container.clientWidth));
    this.cursorEl.height = Math.max(64, Math.floor(this.container.clientHeight));
    this.cursorEl.style.zIndex = '2';
    const cursorCtx = this.cursorEl.getContext('2d');
    this.container.style.position = 'relative';
    this.container.appendChild(this.cursorEl);
    this.cursorRuntime = new SessionCursorRuntime({
      canvas: this.canvas,
      cursorEl: this.cursorEl,
      cursorCtx,
    });

    if ((input.renderBackend ?? 'auto') !== 'canvas2d') {
      const webgl = (input.createWebGLRenderer ?? WebGLTileRenderer.tryCreate)(this.canvas);
      this.glRenderer = webgl.renderer;
      this.renderDiagnostics = webgl.diagnostics;
    } else {
      this.renderDiagnostics = {
        backend: 'canvas2d',
        renderer: null,
        vendor: null,
        software: false,
        reason: 'forced-canvas2d',
      };
    }

    let ctx: CanvasRenderingContext2D | null = null;
    if (this.glRenderer) {
      this.tileCompositor.setWebGLRenderer(this.glRenderer);
    } else {
      ctx = this.canvas.getContext('2d', {
        alpha: false,
        desynchronized: true,
      });
      if (ctx) {
        this.tileCompositor.setContext(ctx);
      }
    }
    this.ctx = ctx;

    this.tileCompositor.setCacheMissHandler(input.onTileCacheMiss);

    this.videoDisplayRuntime = new SessionVideoDisplayRuntime({
      canvas: this.canvas,
      ctx,
      glRenderer: this.glRenderer,
      getGridConfig: () => this.tileCompositor.getGridConfig(),
      getVideoRegion: () => this.tileCompositor.getVideoRegion(),
    });

    const resizeObserverFactory = input.createResizeObserver
      ?? ((callback: ResizeObserverCallback) => new ResizeObserver(callback));
    const resizeObserver = resizeObserverFactory((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        this.resizeRuntime.handleResize(Math.floor(width), Math.floor(height));
      }
    });

    this.resizeRuntime = new SessionResizeRuntime({
      container: this.container,
      canvas: this.canvas,
      cursorEl: this.cursorEl,
      hiDpi: input.hiDpi,
      resizeObserver,
      resizeRenderer: (width, height) => {
        if (this.glRenderer) {
          this.glRenderer.resize(width, height);
        }
      },
      markDisplayDirty: () => {
        this.videoDisplayRuntime.markDirty();
      },
      sendResizeRequest: input.sendResizeRequest,
      setRemoteSize: input.setRemoteSize,
      onResolutionChange: input.onResolutionChange,
    });

    this.resizeRuntime.initializeCanvasSize();
    resizeObserver.observe(this.container);
  }

  start(): void {
    this.videoDisplayRuntime.start();
  }

  getCanvas(): HTMLCanvasElement {
    return this.canvas;
  }

  getRenderDiagnostics(): WebGLRendererDiagnostics {
    return { ...this.renderDiagnostics };
  }

  getContainerResizeDims(): { width: number; height: number } {
    return this.resizeRuntime.getContainerResizeDims();
  }

  applyClientAccessState(flags: number, width: number, height: number): void {
    this.resizeRuntime.applyClientAccessState(flags, width, height);
  }

  handleDecodedFrame(frame: VideoFrame, tileInfo: TileInfo | null): void {
    if (!this.glRenderer && !this.ctx) {
      frame.close();
      return;
    }
    this.videoDisplayRuntime.handleDecodedFrame(frame, tileInfo);
  }

  handleCursorPayload(payload: Uint8Array): void {
    if (this.cursorRuntime.handlePayload(payload)) {
      this.videoDisplayRuntime.markDirty();
    }
  }

  drawCursorMove(x: number, y: number): void {
    this.cursorRuntime.drawMove(x, y);
  }

  markDisplayDirty(): void {
    this.videoDisplayRuntime.markDirty();
  }

  clearVideoOverlay(): void {
    this.videoDisplayRuntime.clearVideoOverlay();
  }

  destroy(): void {
    this.resizeRuntime.destroy();
    this.videoDisplayRuntime.destroy();
    this.cursorRuntime.reset();
    this.tileCompositor.setCacheMissHandler(null);
    this.tileCompositor.setWebGLRenderer(null);

    if (this.glRenderer) {
      this.glRenderer.destroy();
      this.glRenderer = null;
    }

    if (this.canvas.parentNode) {
      this.canvas.parentNode.removeChild(this.canvas);
    }
    if (this.cursorEl && this.cursorEl.parentNode) {
      this.cursorEl.parentNode.removeChild(this.cursorEl);
      this.cursorEl = null;
    }
  }
}
