import type { TileInfo } from './nal.js';
import type { TileGridConfig } from './tile-cache.js';
import type { WebGLTileRenderer } from './webgl-compositor.js';

interface VideoRegion {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface SessionVideoDisplayRuntimeInput {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D | null;
  glRenderer: Pick<WebGLTileRenderer, 'uploadVideoFrame' | 'drawCachedVideo' | 'drawCachedVideoCropped'> | null;
  getGridConfig: () => TileGridConfig | null;
  getVideoRegion: () => VideoRegion | null;
  createCanvas?: () => HTMLCanvasElement;
  requestAnimationFrameFn?: typeof requestAnimationFrame;
  now?: () => number;
  staleMs?: number;
}

const DEFAULT_VIDEO_OVERLAY_STALE_MS = 450;

export class SessionVideoDisplayRuntime {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D | null;
  private readonly glRenderer: SessionVideoDisplayRuntimeInput['glRenderer'];
  private readonly getGridConfig: () => TileGridConfig | null;
  private readonly getVideoRegion: () => VideoRegion | null;
  private readonly createCanvas: () => HTMLCanvasElement;
  private readonly requestAnimationFrameFn: typeof requestAnimationFrame;
  private readonly now: () => number;
  private readonly staleMs: number;

  private pendingVideoFrame: VideoFrame | null = null;
  private pendingTileInfo: TileInfo | null = null;
  private videoBuffer: HTMLCanvasElement | null = null;
  private videoBufferCtx: CanvasRenderingContext2D | null = null;
  private videoBufferTileInfo: TileInfo | null = null;
  private lastVideoFrameAtMs = 0;
  private displayLoopRunning = false;
  private displayDirty = false;

  constructor(input: SessionVideoDisplayRuntimeInput) {
    this.canvas = input.canvas;
    this.ctx = input.ctx;
    this.glRenderer = input.glRenderer;
    this.getGridConfig = input.getGridConfig;
    this.getVideoRegion = input.getVideoRegion;
    this.createCanvas = input.createCanvas ?? (() => document.createElement('canvas'));
    this.requestAnimationFrameFn = input.requestAnimationFrameFn ?? requestAnimationFrame;
    this.now = input.now ?? (() => performance.now());
    this.staleMs = input.staleMs ?? DEFAULT_VIDEO_OVERLAY_STALE_MS;
  }

  start(): void {
    if (this.displayLoopRunning) return;
    this.displayLoopRunning = true;
    this.requestAnimationFrameFn(this.loop);
  }

  destroy(): void {
    this.displayLoopRunning = false;
    this.clearVideoOverlay();
  }

  markDirty(): void {
    this.displayDirty = true;
  }

  handleDecodedFrame(frame: VideoFrame, tileInfo: TileInfo | null): void {
    if (!this.ctx && !this.glRenderer) {
      frame.close();
      return;
    }
    if (this.pendingVideoFrame) {
      this.pendingVideoFrame.close();
    }
    this.pendingVideoFrame = frame;
    this.pendingTileInfo = tileInfo;
    this.displayDirty = true;
  }

  clearVideoOverlay(): void {
    if (this.pendingVideoFrame) {
      this.pendingVideoFrame.close();
      this.pendingVideoFrame = null;
    }
    this.pendingTileInfo = null;
    this.videoBuffer = null;
    this.videoBufferCtx = null;
    this.videoBufferTileInfo = null;
    this.lastVideoFrameAtMs = 0;
  }

  private loop = (): void => {
    if (!this.displayDirty) {
      if (this.displayLoopRunning) {
        this.requestAnimationFrameFn(this.loop);
      }
      return;
    }

    this.displayDirty = false;
    this.commitPendingFrame();
    this.drawOverlay();

    if (this.displayLoopRunning) {
      this.requestAnimationFrameFn(this.loop);
    }
  };

  private commitPendingFrame(): void {
    if (!this.pendingVideoFrame) {
      return;
    }

    if (this.glRenderer) {
      this.glRenderer.uploadVideoFrame(this.pendingVideoFrame);
    } else {
      const frameWidth = this.pendingVideoFrame.displayWidth;
      const frameHeight = this.pendingVideoFrame.displayHeight;
      if (!this.videoBuffer || this.videoBuffer.width !== frameWidth || this.videoBuffer.height !== frameHeight) {
        this.videoBuffer = this.createCanvas();
        this.videoBuffer.width = frameWidth;
        this.videoBuffer.height = frameHeight;
        this.videoBufferCtx = this.videoBuffer.getContext('2d');
      }
      if (this.videoBufferCtx) {
        this.videoBufferCtx.drawImage(this.pendingVideoFrame, 0, 0);
      }
    }

    this.pendingVideoFrame.close();
    this.videoBufferTileInfo = this.pendingTileInfo;
    this.lastVideoFrameAtMs = this.now();
    this.pendingVideoFrame = null;
    this.pendingTileInfo = null;
  }

  private drawOverlay(): void {
    const isFresh = this.lastVideoFrameAtMs > 0
      && (this.now() - this.lastVideoFrameAtMs) <= this.staleMs;
    if (!isFresh) {
      return;
    }

    if (this.glRenderer) {
      const tile = this.videoBufferTileInfo;
      if (tile && tile.tileW > 0 && tile.tileH > 0) {
        this.glRenderer.drawCachedVideo(tile.tileX, tile.tileY, tile.tileW, tile.tileH);
        return;
      }
      if (!this.getGridConfig()) {
        this.glRenderer.drawCachedVideo(0, 0, this.canvas.width, this.canvas.height);
        return;
      }
      const region = this.getVideoRegion();
      if (region && region.w > 0 && region.h > 0) {
        this.glRenderer.drawCachedVideoCropped(
          region.x, region.y, region.w, region.h,
          region.x, region.y, region.w, region.h,
        );
      }
      return;
    }

    if (!this.ctx || !this.videoBuffer || !this.videoBufferCtx) {
      return;
    }

    const tile = this.videoBufferTileInfo;
    if (tile && tile.tileW > 0 && tile.tileH > 0) {
      this.ctx.drawImage(
        this.videoBuffer,
        0, 0, this.videoBuffer.width, this.videoBuffer.height,
        tile.tileX, tile.tileY, tile.tileW, tile.tileH,
      );
      return;
    }
    if (!this.getGridConfig()) {
      this.ctx.drawImage(this.videoBuffer, 0, 0, this.canvas.width, this.canvas.height);
      return;
    }
    const region = this.getVideoRegion();
    if (region && region.w > 0 && region.h > 0) {
      this.ctx.drawImage(
        this.videoBuffer,
        region.x, region.y, region.w, region.h,
        region.x, region.y, region.w, region.h,
      );
    }
  }
}
