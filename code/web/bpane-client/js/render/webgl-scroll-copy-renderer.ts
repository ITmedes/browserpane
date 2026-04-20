export type WebGLScrollCopyArgs = {
  canvasWidth: number;
  canvasHeight: number;
  dx: number;
  dy: number;
  regionTop: number;
  regionBottom: number;
  regionRight: number;
  screenW: number;
  screenH: number;
};

export class WebGLScrollCopyRenderer {
  private gl: WebGL2RenderingContext;
  private scrollFramebuffer: WebGLFramebuffer | null = null;
  private scrollTexture: WebGLTexture | null = null;
  private textureWidth = 0;
  private textureHeight = 0;

  constructor(gl: WebGL2RenderingContext) {
    this.gl = gl;
  }

  scrollCopy(args: WebGLScrollCopyArgs): void {
    const {
      canvasWidth,
      canvasHeight,
      dx,
      dy,
      regionTop,
      regionBottom,
      regionRight,
      screenW,
      screenH,
    } = args;

    if (canvasWidth <= 0 || canvasHeight <= 0) return;

    const tx = -dx || 0;
    const ty = -dy || 0;

    this.ensureResources(canvasWidth, canvasHeight);
    if (!this.scrollFramebuffer || !this.scrollTexture) return;

    const hasRegion = regionTop !== 0 || regionBottom !== screenH || regionRight !== screenW;
    const gl = this.gl;

    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, this.scrollFramebuffer);
    if (hasRegion) {
      const glTop = canvasHeight - regionBottom;
      const glBottom = canvasHeight - regionTop;
      gl.blitFramebuffer(
        0, glTop, regionRight, glBottom,
        0, glTop, regionRight, glBottom,
        gl.COLOR_BUFFER_BIT,
        gl.NEAREST,
      );
    } else {
      gl.blitFramebuffer(
        0, 0, canvasWidth, canvasHeight,
        0, 0, canvasWidth, canvasHeight,
        gl.COLOR_BUFFER_BIT,
        gl.NEAREST,
      );
    }
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);

    if (hasRegion) {
      const regionWidth = regionRight;
      const regionHeight = regionBottom - regionTop;
      if (regionWidth <= 0 || regionHeight <= 0) return;

      const srcX = Math.max(0, -tx);
      const srcY = regionTop + Math.max(0, -ty);
      const destX = Math.max(0, tx);
      const destY = regionTop + Math.max(0, ty);
      const srcWidth = regionWidth - Math.abs(tx);
      const srcHeight = regionHeight - Math.abs(ty);

      if (srcWidth > 0 && srcHeight > 0) {
        this.blitScrollPortion({
          canvasHeight,
          srcX,
          srcY,
          srcWidth,
          srcHeight,
          destX,
          destY,
          destWidth: srcWidth,
          destHeight: srcHeight,
        });
      }
      return;
    }

    this.blitScrollPortion({
      canvasHeight,
      srcX: Math.max(0, -tx),
      srcY: Math.max(0, -ty),
      srcWidth: canvasWidth - Math.abs(tx),
      srcHeight: canvasHeight - Math.abs(ty),
      destX: Math.max(0, tx),
      destY: Math.max(0, ty),
      destWidth: canvasWidth - Math.abs(tx),
      destHeight: canvasHeight - Math.abs(ty),
    });
  }

  destroy(): void {
    const gl = this.gl;
    if (this.scrollFramebuffer) {
      gl.deleteFramebuffer(this.scrollFramebuffer);
      this.scrollFramebuffer = null;
    }
    if (this.scrollTexture) {
      gl.deleteTexture(this.scrollTexture);
      this.scrollTexture = null;
    }
  }

  private ensureResources(width: number, height: number): void {
    const gl = this.gl;
    if (this.scrollTexture && this.textureWidth === width && this.textureHeight === height) {
      return;
    }

    if (this.scrollTexture) gl.deleteTexture(this.scrollTexture);
    this.scrollTexture = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, this.scrollTexture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindTexture(gl.TEXTURE_2D, null);

    if (this.scrollFramebuffer) gl.deleteFramebuffer(this.scrollFramebuffer);
    this.scrollFramebuffer = gl.createFramebuffer()!;
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.scrollFramebuffer);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, this.scrollTexture, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);

    this.textureWidth = width;
    this.textureHeight = height;
  }

  private blitScrollPortion(args: {
    canvasHeight: number;
    srcX: number;
    srcY: number;
    srcWidth: number;
    srcHeight: number;
    destX: number;
    destY: number;
    destWidth: number;
    destHeight: number;
  }): void {
    const {
      canvasHeight,
      srcX,
      srcY,
      srcWidth,
      srcHeight,
      destX,
      destY,
      destWidth,
      destHeight,
    } = args;
    const gl = this.gl;

    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, this.scrollFramebuffer);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);

    const glSrcTop = canvasHeight - (srcY + srcHeight);
    const glSrcBottom = canvasHeight - srcY;
    const glDestTop = canvasHeight - (destY + destHeight);
    const glDestBottom = canvasHeight - destY;

    gl.blitFramebuffer(
      srcX, glSrcTop, srcX + srcWidth, glSrcBottom,
      destX, glDestTop, destX + destWidth, glDestBottom,
      gl.COLOR_BUFFER_BIT,
      gl.NEAREST,
    );

    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);
  }
}
