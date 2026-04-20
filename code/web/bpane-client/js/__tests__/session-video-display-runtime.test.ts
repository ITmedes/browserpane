import { describe, expect, it, vi } from 'vitest';
import type { TileInfo } from '../nal.js';
import { SessionVideoDisplayRuntime } from '../session-video-display-runtime.js';

function createTileInfo(): TileInfo {
  return {
    tileX: 10,
    tileY: 20,
    tileW: 30,
    tileH: 40,
    screenW: 320,
    screenH: 180,
  };
}

function flushAnimationFrame(callback: FrameRequestCallback | null): void {
  if (!callback) {
    throw new Error('missing animation frame callback');
  }
  (callback as FrameRequestCallback)(0);
}

describe('SessionVideoDisplayRuntime', () => {
  it('uploads decoded frames to WebGL and redraws the cached tile region', () => {
    let rafCallback: FrameRequestCallback | null = null;
    const requestAnimationFrameFn = vi.fn((callback: FrameRequestCallback) => {
      rafCallback = callback;
      return 1;
    });
    const glRenderer = {
      uploadVideoFrame: vi.fn(),
      drawCachedVideo: vi.fn(() => true),
      drawCachedVideoCropped: vi.fn(() => true),
    };
    const frame = {
      close: vi.fn(),
      displayWidth: 320,
      displayHeight: 180,
    } as unknown as VideoFrame;
    const runtime = new SessionVideoDisplayRuntime({
      canvas: { width: 800, height: 600 } as HTMLCanvasElement,
      ctx: null,
      glRenderer,
      getGridConfig: () => null,
      getVideoRegion: () => null,
      requestAnimationFrameFn,
      now: () => 100,
    });

    runtime.start();
    runtime.handleDecodedFrame(frame, createTileInfo());
    flushAnimationFrame(rafCallback);

    expect(glRenderer.uploadVideoFrame).toHaveBeenCalledWith(frame);
    expect(frame.close).toHaveBeenCalledOnce();
    expect(glRenderer.drawCachedVideo).toHaveBeenCalledWith(10, 20, 30, 40);
  });

  it('buffers decoded frames on Canvas2D and redraws them full-screen without grid config', () => {
    let rafCallback: FrameRequestCallback | null = null;
    const requestAnimationFrameFn = vi.fn((callback: FrameRequestCallback) => {
      rafCallback = callback;
      return 1;
    });
    const videoBufferCtx = {
      drawImage: vi.fn(),
    };
    const videoBuffer = {
      width: 0,
      height: 0,
      getContext: vi.fn(() => videoBufferCtx),
    } as unknown as HTMLCanvasElement;
    const ctx = {
      drawImage: vi.fn(),
    } as unknown as CanvasRenderingContext2D;
    const frame = {
      close: vi.fn(),
      displayWidth: 640,
      displayHeight: 360,
    } as unknown as VideoFrame;
    const runtime = new SessionVideoDisplayRuntime({
      canvas: { width: 800, height: 600 } as HTMLCanvasElement,
      ctx,
      glRenderer: null,
      getGridConfig: () => null,
      getVideoRegion: () => null,
      createCanvas: () => videoBuffer,
      requestAnimationFrameFn,
      now: () => 100,
    });

    runtime.start();
    runtime.handleDecodedFrame(frame, null);
    flushAnimationFrame(rafCallback);

    expect(videoBuffer.width).toBe(640);
    expect(videoBuffer.height).toBe(360);
    expect(videoBufferCtx.drawImage).toHaveBeenCalledWith(frame, 0, 0);
    expect(ctx.drawImage).toHaveBeenCalledWith(videoBuffer, 0, 0, 800, 600);
    expect(frame.close).toHaveBeenCalledOnce();
  });

  it('redraws the configured video region on Canvas2D when grid config is active', () => {
    let rafCallback: FrameRequestCallback | null = null;
    const requestAnimationFrameFn = vi.fn((callback: FrameRequestCallback) => {
      rafCallback = callback;
      return 1;
    });
    const videoBufferCtx = {
      drawImage: vi.fn(),
    };
    const videoBuffer = {
      width: 0,
      height: 0,
      getContext: vi.fn(() => videoBufferCtx),
    } as unknown as HTMLCanvasElement;
    const ctx = {
      drawImage: vi.fn(),
    } as unknown as CanvasRenderingContext2D;
    const frame = {
      close: vi.fn(),
      displayWidth: 640,
      displayHeight: 360,
    } as unknown as VideoFrame;
    const runtime = new SessionVideoDisplayRuntime({
      canvas: { width: 800, height: 600 } as HTMLCanvasElement,
      ctx,
      glRenderer: null,
      getGridConfig: () => ({ tileSize: 64, cols: 1, rows: 1, screenW: 800, screenH: 600 }),
      getVideoRegion: () => ({ x: 100, y: 110, w: 120, h: 130 }),
      createCanvas: () => videoBuffer,
      requestAnimationFrameFn,
      now: () => 100,
    });

    runtime.start();
    runtime.handleDecodedFrame(frame, null);
    flushAnimationFrame(rafCallback);

    expect(ctx.drawImage).toHaveBeenCalledWith(
      videoBuffer,
      100, 110, 120, 130,
      100, 110, 120, 130,
    );
  });

  it('skips overlay redraw after the cached frame becomes stale', () => {
    let now = 100;
    let rafCallback: FrameRequestCallback | null = null;
    const requestAnimationFrameFn = vi.fn((callback: FrameRequestCallback) => {
      rafCallback = callback;
      return 1;
    });
    const glRenderer = {
      uploadVideoFrame: vi.fn(),
      drawCachedVideo: vi.fn(() => true),
      drawCachedVideoCropped: vi.fn(() => true),
    };
    const frame = {
      close: vi.fn(),
      displayWidth: 320,
      displayHeight: 180,
    } as unknown as VideoFrame;
    const runtime = new SessionVideoDisplayRuntime({
      canvas: { width: 800, height: 600 } as HTMLCanvasElement,
      ctx: null,
      glRenderer,
      getGridConfig: () => null,
      getVideoRegion: () => null,
      requestAnimationFrameFn,
      now: () => now,
      staleMs: 100,
    });

    runtime.start();
    runtime.handleDecodedFrame(frame, createTileInfo());
    flushAnimationFrame(rafCallback);
    glRenderer.drawCachedVideo.mockClear();

    now = 250;
    runtime.markDirty();
    flushAnimationFrame(rafCallback);

    expect(glRenderer.drawCachedVideo).not.toHaveBeenCalled();
  });

  it('closes superseded pending frames and clears pending overlay state on reset', () => {
    let rafCallback: FrameRequestCallback | null = null;
    const requestAnimationFrameFn = vi.fn((callback: FrameRequestCallback) => {
      rafCallback = callback;
      return 1;
    });
    const glRenderer = {
      uploadVideoFrame: vi.fn(),
      drawCachedVideo: vi.fn(() => true),
      drawCachedVideoCropped: vi.fn(() => true),
    };
    const first = {
      close: vi.fn(),
      displayWidth: 320,
      displayHeight: 180,
    } as unknown as VideoFrame;
    const second = {
      close: vi.fn(),
      displayWidth: 320,
      displayHeight: 180,
    } as unknown as VideoFrame;
    const runtime = new SessionVideoDisplayRuntime({
      canvas: { width: 800, height: 600 } as HTMLCanvasElement,
      ctx: null,
      glRenderer,
      getGridConfig: () => null,
      getVideoRegion: () => null,
      requestAnimationFrameFn,
      now: () => 100,
    });

    runtime.start();
    runtime.handleDecodedFrame(first, createTileInfo());
    runtime.handleDecodedFrame(second, createTileInfo());
    expect(first.close).toHaveBeenCalledOnce();

    runtime.clearVideoOverlay();
    flushAnimationFrame(rafCallback);

    expect(second.close).toHaveBeenCalledOnce();
    expect(glRenderer.uploadVideoFrame).not.toHaveBeenCalled();
    expect(glRenderer.drawCachedVideo).not.toHaveBeenCalled();
  });

  it('binds the default requestAnimationFrame receiver for browser globals', () => {
    const originalRequestAnimationFrame = window.requestAnimationFrame;
    let capturedCallback: FrameRequestCallback | null = null;
    window.requestAnimationFrame = function (callback: FrameRequestCallback): number {
      if (this !== window) {
        throw new TypeError('Illegal invocation');
      }
      capturedCallback = callback;
      return 1;
    };

    try {
      const runtime = new SessionVideoDisplayRuntime({
        canvas: { width: 800, height: 600 } as HTMLCanvasElement,
        ctx: null,
        glRenderer: {
          uploadVideoFrame: vi.fn(),
          drawCachedVideo: vi.fn(() => true),
          drawCachedVideoCropped: vi.fn(() => true),
        },
        getGridConfig: () => null,
        getVideoRegion: () => null,
      });

      expect(() => runtime.start()).not.toThrow();
      expect(capturedCallback).not.toBeNull();
    } finally {
      window.requestAnimationFrame = originalRequestAnimationFrame;
    }
  });
});
