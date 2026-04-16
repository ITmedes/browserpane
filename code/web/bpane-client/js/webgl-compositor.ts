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
  private tileTexture: WebGLTexture;
  private uRect: WebGLUniformLocation;
  private uResolution: WebGLUniformLocation;
  private uMode: WebGLUniformLocation;
  private uColor: WebGLUniformLocation;
  private vao: WebGLVertexArrayObject;
  private quadBuffer: WebGLBuffer;

  private cachedVideoRenderer: WebGLCachedVideoRenderer;
  private scrollCopyRenderer: WebGLScrollCopyRenderer;

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

    // Create the reusable tile texture
    this.tileTexture = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, this.tileTexture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindTexture(gl.TEXTURE_2D, null);

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
    const gl = this.gl;
    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);

    // Upload ImageData to the reusable tile texture
    gl.bindTexture(gl.TEXTURE_2D, this.tileTexture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, imageData);

    gl.uniform4f(this.uRect, x, y, w, h);
    gl.uniform1i(this.uMode, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);

    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
  }

  /** Draw a tile from an ImageBitmap at pixel coordinates (zero-copy on Chrome). */
  drawTileImageBitmap(x: number, y: number, w: number, h: number, bitmap: ImageBitmap): void {
    const gl = this.gl;
    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);

    gl.bindTexture(gl.TEXTURE_2D, this.tileTexture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, bitmap);

    gl.uniform4f(this.uRect, x, y, w, h);
    gl.uniform1i(this.uMode, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);

    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
  }

  /**
   * Draw a VideoFrame at pixel coordinates (zero-copy GPU path on Chrome).
   * The caller is responsible for closing the VideoFrame after this call.
   */
  drawVideoFrame(x: number, y: number, w: number, h: number, frame: VideoFrame): void {
    const gl = this.gl;
    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);

    gl.bindTexture(gl.TEXTURE_2D, this.tileTexture);
    // VideoFrame is accepted as a TexImageSource in Chrome
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, frame as any);

    gl.uniform4f(this.uRect, x, y, w, h);
    gl.uniform1i(this.uMode, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);

    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
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
    const gl = this.gl;
    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);

    gl.bindTexture(gl.TEXTURE_2D, this.tileTexture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);

    gl.uniform4f(this.uRect, x, y, w, h);
    gl.uniform1i(this.uMode, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);

    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
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
    const gl = this.gl;
    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);

    gl.bindTexture(gl.TEXTURE_2D, this.tileTexture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);

    // Use scissor test to clip to the destination rect, and adjust quad to
    // handle the source sub-rect. The simplest approach: draw the full source
    // scaled/positioned so the source sub-rect lands on the dest rect.
    //
    // Full-source rect would cover:
    //   destX - (srcX/srcW)*destW, destY - (srcY/srcH)*destH
    //   with size: (sourceWidth/srcW)*destW, (sourceHeight/srcH)*destH
    const fullW = (sourceWidth / srcW) * destW;
    const fullH = (sourceHeight / srcH) * destH;
    const fullX = destX - (srcX / srcW) * destW;
    const fullY = destY - (srcY / srcH) * destH;

    gl.enable(gl.SCISSOR_TEST);
    // Scissor Y is in GL coords (bottom-up)
    const canvasH = this.canvasH;
    gl.scissor(destX, canvasH - (destY + destH), destW, destH);

    gl.uniform4f(this.uRect, fullX, fullY, fullW, fullH);
    gl.uniform1i(this.uMode, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);

    gl.disable(gl.SCISSOR_TEST);
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
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
    gl.deleteTexture(this.tileTexture);
    gl.deleteBuffer(this.quadBuffer);
    gl.deleteVertexArray(this.vao);
    gl.deleteProgram(this.program);
  }
}
