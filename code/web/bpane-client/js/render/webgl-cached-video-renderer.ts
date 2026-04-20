export type WebGLCachedVideoBindings = {
  program: WebGLProgram;
  vao: WebGLVertexArrayObject;
  uRect: WebGLUniformLocation;
  uMode: WebGLUniformLocation;
};

export class WebGLCachedVideoRenderer {
  private gl: WebGL2RenderingContext;
  private bindings: WebGLCachedVideoBindings;
  private texture: WebGLTexture | null = null;
  private textureWidth = 0;
  private textureHeight = 0;
  private valid = false;
  private canvasHeight = 0;

  constructor(gl: WebGL2RenderingContext, bindings: WebGLCachedVideoBindings) {
    this.gl = gl;
    this.bindings = bindings;
  }

  resize(canvasHeight: number): void {
    this.canvasHeight = canvasHeight;
  }

  upload(frame: VideoFrame): void {
    const gl = this.gl;
    const frameWidth = frame.displayWidth;
    const frameHeight = frame.displayHeight;

    if (!this.texture || this.textureWidth !== frameWidth || this.textureHeight !== frameHeight) {
      if (this.texture) gl.deleteTexture(this.texture);
      this.texture = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, this.texture);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      this.textureWidth = frameWidth;
      this.textureHeight = frameHeight;
    } else {
      gl.bindTexture(gl.TEXTURE_2D, this.texture);
    }

    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, frame as any);
    gl.bindTexture(gl.TEXTURE_2D, null);
    this.valid = true;
  }

  draw(x: number, y: number, width: number, height: number): boolean {
    if (!this.texture || !this.valid) return false;

    const gl = this.gl;
    gl.useProgram(this.bindings.program);
    gl.bindVertexArray(this.bindings.vao);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.uniform4f(this.bindings.uRect, x, y, width, height);
    gl.uniform1i(this.bindings.uMode, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
    return true;
  }

  drawCropped(
    _srcX: number,
    _srcY: number,
    _srcWidth: number,
    _srcHeight: number,
    destX: number,
    destY: number,
    destWidth: number,
    destHeight: number,
  ): boolean {
    if (!this.texture || !this.valid) return false;

    const gl = this.gl;
    gl.useProgram(this.bindings.program);
    gl.bindVertexArray(this.bindings.vao);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.enable(gl.SCISSOR_TEST);
    gl.scissor(destX, this.canvasHeight - destY - destHeight, destWidth, destHeight);
    gl.uniform4f(this.bindings.uRect, destX, destY, destWidth, destHeight);
    gl.uniform1i(this.bindings.uMode, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.disable(gl.SCISSOR_TEST);
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
    return true;
  }

  invalidate(): void {
    this.valid = false;
  }

  destroy(): void {
    if (!this.texture) return;
    this.gl.deleteTexture(this.texture);
    this.texture = null;
    this.valid = false;
  }
}
