export type WebGLTileProgram = {
  program: WebGLProgram;
  uRect: WebGLUniformLocation;
  uResolution: WebGLUniformLocation;
  uMode: WebGLUniformLocation;
  uColor: WebGLUniformLocation;
  vao: WebGLVertexArrayObject;
  quadBuffer: WebGLBuffer;
};

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

export function createWebGLTileProgram(gl: WebGL2RenderingContext): WebGLTileProgram {
  const vertexShader = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER_SRC);
  try {
    const fragmentShader = compileShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER_SRC);
    try {
      const program = linkProgram(gl, vertexShader, fragmentShader);
      gl.useProgram(program);

      const uRect = gl.getUniformLocation(program, 'u_rect')!;
      const uResolution = gl.getUniformLocation(program, 'u_resolution')!;
      const uMode = gl.getUniformLocation(program, 'u_mode')!;
      const uColor = gl.getUniformLocation(program, 'u_color')!;

      const quadData = new Float32Array([
        0, 0, 0, 0,
        1, 0, 1, 0,
        0, 1, 0, 1,
        0, 1, 0, 1,
        1, 0, 1, 0,
        1, 1, 1, 1,
      ]);

      const vao = gl.createVertexArray()!;
      gl.bindVertexArray(vao);

      const quadBuffer = gl.createBuffer()!;
      gl.bindBuffer(gl.ARRAY_BUFFER, quadBuffer);
      gl.bufferData(gl.ARRAY_BUFFER, quadData, gl.STATIC_DRAW);

      const aPos = gl.getAttribLocation(program, 'a_position');
      const aTex = gl.getAttribLocation(program, 'a_texCoord');

      gl.enableVertexAttribArray(aPos);
      gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 16, 0);
      gl.enableVertexAttribArray(aTex);
      gl.vertexAttribPointer(aTex, 2, gl.FLOAT, false, 16, 8);

      gl.bindVertexArray(null);

      return {
        program,
        uRect,
        uResolution,
        uMode,
        uColor,
        vao,
        quadBuffer,
      };
    } finally {
      gl.deleteShader(fragmentShader);
    }
  } finally {
    gl.deleteShader(vertexShader);
  }
}
