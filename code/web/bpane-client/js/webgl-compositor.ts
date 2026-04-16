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

// ── Shader sources ──────────────────────────────────────────────────

const VERTEX_SHADER_SRC = `#version 300 es
in vec2 a_position;
in vec2 a_texCoord;
out vec2 v_texCoord;
uniform vec4 u_rect;       // (x, y, w, h) in pixels
uniform vec2 u_resolution;  // canvas size
void main() {
  vec2 pos = u_rect.xy + a_position * u_rect.zw;
  vec2 clip = (pos / u_resolution) * 2.0 - 1.0;
  clip.y = -clip.y; // flip Y — canvas origin is top-left
  gl_Position = vec4(clip, 0.0, 1.0);
  v_texCoord = a_texCoord;
}
`;

const FRAGMENT_SHADER_SRC = `#version 300 es
precision mediump float;
in vec2 v_texCoord;
out vec4 fragColor;
uniform sampler2D u_texture;
uniform int u_mode; // 0 = texture, 1 = solid color
uniform vec4 u_color;
void main() {
  if (u_mode == 1) {
    fragColor = u_color;
  } else {
    fragColor = texture(u_texture, v_texCoord);
  }
}
`;

// ── Helper: compile shader ──────────────────────────────────────────

function compileShader(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error('Failed to create shader');
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error(`Shader compile error: ${log}`);
  }
  return shader;
}

function linkProgram(gl: WebGL2RenderingContext, vs: WebGLShader, fs: WebGLShader): WebGLProgram {
  const program = gl.createProgram();
  if (!program) throw new Error('Failed to create program');
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(program);
    gl.deleteProgram(program);
    throw new Error(`Program link error: ${log}`);
  }
  return program;
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

  // Persistent video texture — caches last uploaded video frame on the GPU
  // so re-compositing doesn't require a CPU round-trip.
  private videoTexture: WebGLTexture | null = null;
  private videoTexW = 0;
  private videoTexH = 0;
  private videoTexValid = false;

  // Scroll copy resources (lazy-initialized)
  private scrollFbo: WebGLFramebuffer | null = null;
  private scrollTexture: WebGLTexture | null = null;
  private scrollTexW = 0;
  private scrollTexH = 0;

  // Current canvas dimensions (set via resize())
  private canvasW = 0;
  private canvasH = 0;

  constructor(gl: WebGL2RenderingContext, info: WebGLContextInfo = detectContextInfo(gl)) {
    this.gl = gl;
    this.info = info;

    // Compile shaders and link program
    const vs = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER_SRC);
    const fs = compileShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER_SRC);
    this.program = linkProgram(gl, vs, fs);
    // Shaders can be deleted after linking
    gl.deleteShader(vs);
    gl.deleteShader(fs);

    gl.useProgram(this.program);

    // Uniform locations
    this.uRect = gl.getUniformLocation(this.program, 'u_rect')!;
    this.uResolution = gl.getUniformLocation(this.program, 'u_resolution')!;
    this.uMode = gl.getUniformLocation(this.program, 'u_mode')!;
    this.uColor = gl.getUniformLocation(this.program, 'u_color')!;

    // Create a unit quad (positions 0..1, texcoords 0..1)
    // Two triangles covering a unit square
    const quadData = new Float32Array([
      // position (x,y), texCoord (u,v)
      0, 0, 0, 0,
      1, 0, 1, 0,
      0, 1, 0, 1,
      0, 1, 0, 1,
      1, 0, 1, 0,
      1, 1, 1, 1,
    ]);

    this.vao = gl.createVertexArray()!;
    gl.bindVertexArray(this.vao);

    this.quadBuffer = gl.createBuffer()!;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.quadBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, quadData, gl.STATIC_DRAW);

    const aPos = gl.getAttribLocation(this.program, 'a_position');
    const aTex = gl.getAttribLocation(this.program, 'a_texCoord');

    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 16, 0);
    gl.enableVertexAttribArray(aTex);
    gl.vertexAttribPointer(aTex, 2, gl.FLOAT, false, 16, 8);

    gl.bindVertexArray(null);

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
    const gl = this.gl;
    const fw = frame.displayWidth;
    const fh = frame.displayHeight;
    if (!this.videoTexture || this.videoTexW !== fw || this.videoTexH !== fh) {
      if (this.videoTexture) gl.deleteTexture(this.videoTexture);
      this.videoTexture = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, this.videoTexture);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      this.videoTexW = fw;
      this.videoTexH = fh;
    } else {
      gl.bindTexture(gl.TEXTURE_2D, this.videoTexture);
    }
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, frame as any);
    gl.bindTexture(gl.TEXTURE_2D, null);
    this.videoTexValid = true;
  }

  /**
   * Draw the cached video texture at pixel coordinates.
   * Returns false if no video texture has been uploaded yet.
   */
  drawCachedVideo(x: number, y: number, w: number, h: number): boolean {
    if (!this.videoTexture || !this.videoTexValid) return false;
    const gl = this.gl;
    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);
    gl.bindTexture(gl.TEXTURE_2D, this.videoTexture);
    gl.uniform4f(this.uRect, x, y, w, h);
    gl.uniform1i(this.uMode, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
    return true;
  }

  /**
   * Draw a sub-rect of the cached video texture at a destination rect.
   * Returns false if no video texture has been uploaded yet.
   */
  drawCachedVideoCropped(
    srcX: number, srcY: number, srcW: number, srcH: number,
    dstX: number, dstY: number, dstW: number, dstH: number,
  ): boolean {
    if (!this.videoTexture || !this.videoTexValid) return false;
    const gl = this.gl;
    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);
    gl.bindTexture(gl.TEXTURE_2D, this.videoTexture);
    // Use scissor to crop, draw full texture at destination
    gl.enable(gl.SCISSOR_TEST);
    gl.scissor(dstX, this.canvasH - dstY - dstH, dstW, dstH);
    gl.uniform4f(this.uRect, dstX, dstY, dstW, dstH);
    gl.uniform1i(this.uMode, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.disable(gl.SCISSOR_TEST);
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
    return true;
  }

  /** Invalidate the cached video texture (e.g., on disconnect). */
  invalidateVideoTexture(): void {
    this.videoTexValid = false;
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
    const gl = this.gl;
    const cw = this.canvasW;
    const ch = this.canvasH;
    if (cw <= 0 || ch <= 0) return;

    // Negate: server sends scroll direction, we shift pixels in the opposite direction
    const tx = -dx || 0;
    const ty = -dy || 0;

    // Ensure scroll FBO/texture exist and are the right size
    this.ensureScrollResources(cw, ch);
    if (!this.scrollFbo || !this.scrollTexture) return;

    const hasRegion = regionTop !== 0 || regionBottom !== screenH || regionRight !== screenW;

    // Step 1: Copy current framebuffer to the scroll texture.
    // Only blit the scroll region when available (avoids full-framebuffer copy).
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, this.scrollFbo);
    if (hasRegion) {
      // Blit only the viewport region (GL Y is bottom-up)
      const glTop = ch - regionBottom;
      const glBot = ch - regionTop;
      gl.blitFramebuffer(
        0, glTop, regionRight, glBot,
        0, glTop, regionRight, glBot,
        gl.COLOR_BUFFER_BIT,
        gl.NEAREST,
      );
    } else {
      gl.blitFramebuffer(
        0, 0, cw, ch,
        0, 0, cw, ch,
        gl.COLOR_BUFFER_BIT,
        gl.NEAREST,
      );
    }
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);

    // Step 2: Redraw the shifted portion over the existing framebuffer.
    if (hasRegion) {
      const rw = regionRight;
      const rh = regionBottom - regionTop;
      if (rw <= 0 || rh <= 0) return;

      // Compute clipped source/dest regions
      const srcX = Math.max(0, -tx);
      const srcY = regionTop + Math.max(0, -ty);
      const destX = Math.max(0, tx);
      const destY = regionTop + Math.max(0, ty);
      const srcW = rw - Math.abs(tx);
      const srcH = rh - Math.abs(ty);

      if (srcW > 0 && srcH > 0) {
        this.drawScrollTexturePortion(srcX, srcY, srcW, srcH, destX, destY, srcW, srcH);
      }
    } else {
      // Full-screen scroll — redraw the shifted framebuffer content.
      this.drawScrollTexturePortion(
        Math.max(0, -tx), Math.max(0, -ty),
        cw - Math.abs(tx), ch - Math.abs(ty),
        Math.max(0, tx), Math.max(0, ty),
        cw - Math.abs(tx), ch - Math.abs(ty),
      );
    }
  }

  /** Clear the entire canvas. */
  clear(): void {
    this.gl.clear(this.gl.COLOR_BUFFER_BIT);
  }

  /** Release GPU resources. Call on disconnect/cleanup. */
  destroy(): void {
    const gl = this.gl;
    if (this.scrollFbo) { gl.deleteFramebuffer(this.scrollFbo); this.scrollFbo = null; }
    if (this.scrollTexture) { gl.deleteTexture(this.scrollTexture); this.scrollTexture = null; }
    if (this.videoTexture) { gl.deleteTexture(this.videoTexture); this.videoTexture = null; }
    gl.deleteTexture(this.tileTexture);
    gl.deleteBuffer(this.quadBuffer);
    gl.deleteVertexArray(this.vao);
    gl.deleteProgram(this.program);
  }

  // ── Private helpers ──────────────────────────────────────────────

  /**
   * Ensure the scroll framebuffer + texture exist and match canvas size.
   */
  private ensureScrollResources(width: number, height: number): void {
    const gl = this.gl;
    if (this.scrollTexture && this.scrollTexW === width && this.scrollTexH === height) {
      return;
    }

    // (Re)create texture
    if (this.scrollTexture) gl.deleteTexture(this.scrollTexture);
    this.scrollTexture = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, this.scrollTexture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindTexture(gl.TEXTURE_2D, null);

    // (Re)create framebuffer
    if (this.scrollFbo) gl.deleteFramebuffer(this.scrollFbo);
    this.scrollFbo = gl.createFramebuffer()!;
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.scrollFbo);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, this.scrollTexture, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);

    this.scrollTexW = width;
    this.scrollTexH = height;
  }

  /**
   * Draw a portion of the scroll texture onto the default framebuffer.
   * Coordinates are in canvas-space (top-left origin, Y-down).
   */
  private drawScrollTexturePortion(
    srcX: number, srcY: number, srcW: number, srcH: number,
    destX: number, destY: number, destW: number, destH: number,
  ): void {
    const gl = this.gl;
    const cw = this.canvasW;
    const ch = this.canvasH;

    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);

    // Bind the scroll texture (not the tile texture)
    gl.bindTexture(gl.TEXTURE_2D, this.scrollTexture);

    // We need custom tex coords that sample the source region from the scroll texture.
    // The scroll texture is a copy of the framebuffer which has OpenGL Y orientation
    // (bottom row = row 0), but our texImage2D copied it via blitFramebuffer which
    // preserves orientation. So the texture has GL orientation.
    //
    // Our vertex shader transforms a_position (0..1) into clip space using u_rect.
    // The a_texCoord (0..1) goes straight to the fragment shader.
    // We need to remap tex coords from the full-quad 0..1 to the source sub-rect.
    //
    // Instead of modifying the VBO, we use a simpler approach:
    // Set u_rect to the destination rect, and use a second draw with adjusted
    // texture coordinates via a separate uniform.
    //
    // Actually, the simplest approach is to use blitFramebuffer for the scroll copy
    // since we already have the content in the FBO. Let's do that.

    gl.bindVertexArray(null);
    gl.bindTexture(gl.TEXTURE_2D, null);

    // Use blitFramebuffer from the scroll FBO to the default framebuffer
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, this.scrollFbo);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);

    // Convert canvas-space (Y-down) coordinates to GL-space (Y-up) for blitFramebuffer
    const glSrcY0 = ch - (srcY + srcH);
    const glSrcY1 = ch - srcY;
    const glDstY0 = ch - (destY + destH);
    const glDstY1 = ch - destY;

    gl.blitFramebuffer(
      srcX, glSrcY0, srcX + srcW, glSrcY1,
      destX, glDstY0, destX + destW, glDstY1,
      gl.COLOR_BUFFER_BIT,
      gl.NEAREST,
    );

    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);
  }
}
