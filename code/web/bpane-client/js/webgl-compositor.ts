/**
 * WebGL2-based tile compositor for GPU-accelerated rendering.
 *
 * Replaces Canvas2D putImageData / fillRect / drawImage with textured quads
 * drawn via a minimal vertex + fragment shader pair.
 *
 * Design:
 * - A single reusable texture is uploaded per tile draw (texImage2D).
 * - Solid-color fills use a uniform color (no texture upload).
 * - Scroll copies use a framebuffer blit approach (read to temp texture, redraw shifted).
 * - preserveDrawingBuffer: true — tiles are drawn incrementally, not redrawn every frame.
 */

import {
  detectContextInfo,
  selectWebGLContext,
  type WebGLContextInfo,
  type WebGLRendererDiagnostics,
} from './render/webgl-context-selection.js';
import { WebGLCachedVideoRenderer } from './render/webgl-cached-video-renderer.js';
import { WebGLScrollCopyRenderer } from './render/webgl-scroll-copy-renderer.js';
import { WebGLTextureSourceRenderer } from './render/webgl-texture-source-renderer.js';
import { createWebGLTileProgram } from './render/webgl-tile-program.js';

export type {
  RenderBackend,
  RenderSelectionReason,
  WebGLContextInfo,
  WebGLRendererDiagnostics,
} from './render/webgl-context-selection.js';

export interface WebGLRendererCreationResult {
  renderer: WebGLTileRenderer | null;
  diagnostics: WebGLRendererDiagnostics;
}

// ── WebGLTileRenderer ───────────────────────────────────────────────

export class WebGLTileRenderer {
  private gl: WebGL2RenderingContext;
  private info: WebGLContextInfo;
  private program: WebGLProgram;
  private uRect: WebGLUniformLocation;
  private uResolution: WebGLUniformLocation;
  private uMode: WebGLUniformLocation;
  private uColor: WebGLUniformLocation;
  private vao: WebGLVertexArrayObject;
  private quadBuffer: WebGLBuffer;

  private cachedVideoRenderer: WebGLCachedVideoRenderer;
  private scrollCopyRenderer: WebGLScrollCopyRenderer;
  private textureSourceRenderer: WebGLTextureSourceRenderer;

  // Current canvas dimensions (set via resize())
  private canvasW = 0;
  private canvasH = 0;

  constructor(gl: WebGL2RenderingContext, info: WebGLContextInfo = detectContextInfo(gl)) {
    this.gl = gl;
    this.info = info;

    const tileProgram = createWebGLTileProgram(gl);
    this.program = tileProgram.program;
    this.uRect = tileProgram.uRect;
    this.uResolution = tileProgram.uResolution;
    this.uMode = tileProgram.uMode;
    this.uColor = tileProgram.uColor;
    this.vao = tileProgram.vao;
    this.quadBuffer = tileProgram.quadBuffer;
    this.cachedVideoRenderer = new WebGLCachedVideoRenderer(gl, {
      program: this.program,
      vao: this.vao,
      uRect: this.uRect,
      uMode: this.uMode,
    });
    this.scrollCopyRenderer = new WebGLScrollCopyRenderer(gl);
    this.textureSourceRenderer = new WebGLTextureSourceRenderer(gl, {
      program: this.program,
      vao: this.vao,
      uRect: this.uRect,
      uMode: this.uMode,
    });

    // Disable blending — opaque tiles, no transparency on main canvas
    gl.disable(gl.BLEND);
    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.SCISSOR_TEST);

    // Clear color
    gl.clearColor(0, 0, 0, 1);
  }

  /** WebGL renderer diagnostics for the active context. */
  getContextInfo(): WebGLContextInfo {
    return { ...this.info };
  }

  /**
   * Try to create a WebGL2 context from a canvas.
   * Rejects software-backed contexts and reports why selection fell back.
   */
  static tryCreate(canvas: HTMLCanvasElement): WebGLRendererCreationResult {
    const selection = selectWebGLContext(canvas);
    if (!selection.gl) {
      return {
        renderer: null,
        diagnostics: selection.diagnostics,
      };
    }

    return {
      renderer: new WebGLTileRenderer(selection.gl, {
        renderer: selection.diagnostics.renderer,
        vendor: selection.diagnostics.vendor,
        software: selection.diagnostics.software,
      }),
      diagnostics: selection.diagnostics,
    };
  }

  /** Update viewport and resolution uniform after canvas resize. */
  resize(width: number, height: number): void {
    this.canvasW = width;
    this.canvasH = height;
    this.cachedVideoRenderer.resize(height);
    this.textureSourceRenderer.resize(height);
    const gl = this.gl;
    gl.viewport(0, 0, width, height);
    gl.useProgram(this.program);
    gl.uniform2f(this.uResolution, width, height);
  }

  /** Draw a solid-color rectangle at pixel coordinates. */
  drawFill(x: number, y: number, w: number, h: number, r: number, g: number, b: number, a: number): void {
    const gl = this.gl;
    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);
    gl.uniform4f(this.uRect, x, y, w, h);
    gl.uniform1i(this.uMode, 1);
    gl.uniform4f(this.uColor, r / 255, g / 255, b / 255, a);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.bindVertexArray(null);
  }

  /** Draw a tile from ImageData at pixel coordinates. */
  drawTileImageData(x: number, y: number, w: number, h: number, imageData: ImageData): void {
    this.textureSourceRenderer.draw(x, y, w, h, imageData);
  }

  /** Draw a tile from an ImageBitmap at pixel coordinates (zero-copy on Chrome). */
  drawTileImageBitmap(x: number, y: number, w: number, h: number, bitmap: ImageBitmap): void {
    this.textureSourceRenderer.draw(x, y, w, h, bitmap);
  }

  /**
   * Draw a VideoFrame at pixel coordinates (zero-copy GPU path on Chrome).
   * The caller is responsible for closing the VideoFrame after this call.
   */
  drawVideoFrame(x: number, y: number, w: number, h: number, frame: VideoFrame): void {
    this.textureSourceRenderer.draw(x, y, w, h, frame);
  }

  /**
   * Upload a VideoFrame to the persistent GPU video texture (zero-copy on Chrome).
   * The caller must close the VideoFrame after this call.
   */
  uploadVideoFrame(frame: VideoFrame): void {
    this.cachedVideoRenderer.upload(frame);
  }

  /**
   * Draw the cached video texture at pixel coordinates.
   * Returns false if no video texture has been uploaded yet.
   */
  drawCachedVideo(x: number, y: number, w: number, h: number): boolean {
    return this.cachedVideoRenderer.draw(x, y, w, h);
  }

  /**
   * Draw a sub-rect of the cached video texture at a destination rect.
   * Returns false if no video texture has been uploaded yet.
   */
  drawCachedVideoCropped(
    srcX: number, srcY: number, srcW: number, srcH: number,
    dstX: number, dstY: number, dstW: number, dstH: number,
  ): boolean {
    return this.cachedVideoRenderer.drawCropped(srcX, srcY, srcW, srcH, dstX, dstY, dstW, dstH);
  }

  /** Invalidate the cached video texture (e.g., on disconnect). */
  invalidateVideoTexture(): void {
    this.cachedVideoRenderer.invalidate();
  }

  /**
   * Draw any TexImageSource (HTMLCanvasElement, ImageBitmap, VideoFrame, etc.)
   * at pixel coordinates. Useful for compositing the video overlay buffer.
   */
  drawTexImageSource(x: number, y: number, w: number, h: number, source: TexImageSource): void {
    this.textureSourceRenderer.draw(x, y, w, h, source);
  }

  /**
   * Draw a sub-rectangle of a TexImageSource at a destination position.
   * Used for cropping video frames to a region.
   *
   * srcX, srcY, srcW, srcH: source rectangle in source pixel coordinates
   * destX, destY, destW, destH: destination rectangle on canvas
   * sourceWidth, sourceHeight: full source dimensions (needed for tex coord math)
   */
  drawTexImageSourceCropped(
    source: TexImageSource,
    srcX: number, srcY: number, srcW: number, srcH: number,
    destX: number, destY: number, destW: number, destH: number,
    sourceWidth: number, sourceHeight: number,
  ): void {
    this.textureSourceRenderer.drawCropped(
      source,
      srcX,
      srcY,
      srcW,
      srcH,
      destX,
      destY,
      destW,
      destH,
      sourceWidth,
      sourceHeight,
    );
  }

  /**
   * Scroll copy: shift existing framebuffer pixels by (dx, dy) within a region.
   *
 * Strategy: copy the current framebuffer to a temporary texture, then redraw
 * the shifted portion over the existing framebuffer. The newly exposed strip
 * stays stale for a moment instead of flashing the clear color while repair
 * tiles are still in flight.
   */
  scrollCopy(
    dx: number,
    dy: number,
    regionTop: number,
    regionBottom: number,
    regionRight: number,
    screenW: number,
    screenH: number,
  ): void {
    this.scrollCopyRenderer.scrollCopy({
      canvasWidth: this.canvasW,
      canvasHeight: this.canvasH,
      dx,
      dy,
      regionTop,
      regionBottom,
      regionRight,
      screenW,
      screenH,
    });
  }

  /** Clear the entire canvas. */
  clear(): void {
    this.gl.clear(this.gl.COLOR_BUFFER_BIT);
  }

  /** Release GPU resources. Call on disconnect/cleanup. */
  destroy(): void {
    const gl = this.gl;
    this.cachedVideoRenderer.destroy();
    this.scrollCopyRenderer.destroy();
    this.textureSourceRenderer.destroy();
    gl.deleteBuffer(this.quadBuffer);
    gl.deleteVertexArray(this.vao);
    gl.deleteProgram(this.program);
  }
}
