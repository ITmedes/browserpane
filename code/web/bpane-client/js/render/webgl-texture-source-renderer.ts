export type WebGLTextureSourceBindings = {
  program: WebGLProgram;
  vao: WebGLVertexArrayObject;
  uRect: WebGLUniformLocation;
  uMode: WebGLUniformLocation;
};

export class WebGLTextureSourceRenderer {
  private gl: WebGL2RenderingContext;
  private bindings: WebGLTextureSourceBindings;
  private texture: WebGLTexture;
  private canvasHeight = 0;

  constructor(gl: WebGL2RenderingContext, bindings: WebGLTextureSourceBindings) {
    this.gl = gl;
    this.bindings = bindings;
    this.texture = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindTexture(gl.TEXTURE_2D, null);
  }

  resize(canvasHeight: number): void {
    this.canvasHeight = canvasHeight;
  }

  draw(x: number, y: number, width: number, height: number, source: TexImageSource | ImageData): void {
    const gl = this.gl;
    gl.useProgram(this.bindings.program);
    gl.bindVertexArray(this.bindings.vao);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source as any);
    gl.uniform4f(this.bindings.uRect, x, y, width, height);
    gl.uniform1i(this.bindings.uMode, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
  }

  drawCropped(
    source: TexImageSource,
    srcX: number,
    srcY: number,
    srcWidth: number,
    srcHeight: number,
    destX: number,
    destY: number,
    destWidth: number,
    destHeight: number,
    sourceWidth: number,
    sourceHeight: number,
  ): void {
    const gl = this.gl;
    gl.useProgram(this.bindings.program);
    gl.bindVertexArray(this.bindings.vao);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);

    const fullWidth = (sourceWidth / srcWidth) * destWidth;
    const fullHeight = (sourceHeight / srcHeight) * destHeight;
    const fullX = destX - (srcX / srcWidth) * destWidth;
    const fullY = destY - (srcY / srcHeight) * destHeight;

    gl.enable(gl.SCISSOR_TEST);
    gl.scissor(destX, this.canvasHeight - (destY + destHeight), destWidth, destHeight);
    gl.uniform4f(this.bindings.uRect, fullX, fullY, fullWidth, fullHeight);
    gl.uniform1i(this.bindings.uMode, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.disable(gl.SCISSOR_TEST);
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
  }

  destroy(): void {
    this.gl.deleteTexture(this.texture);
  }
}
