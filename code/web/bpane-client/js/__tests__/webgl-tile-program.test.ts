import { afterEach, describe, expect, it, vi } from 'vitest';

import { createWebGLTileProgram } from '../render/webgl-tile-program.js';

type ProgramGlOptions = {
  createShaderResults?: Array<WebGLShader | null>;
  compileStatuses?: boolean[];
  shaderInfoLog?: string;
  programLinkStatus?: boolean;
  programInfoLog?: string;
};

function createProgramGl(options: ProgramGlOptions = {}): WebGL2RenderingContext {
  const createShaderResults = [...(options.createShaderResults ?? [{ id: 'vs' }, { id: 'fs' }])];
  const compileStatuses = [...(options.compileStatuses ?? [true, true])];
  const vertexShader = createShaderResults[0] ?? { id: 'missing-vs' };
  const fragmentShader = createShaderResults[1] ?? { id: 'missing-fs' };
  const program = { id: 'program' };

  return {
    VERTEX_SHADER: 0x8B31,
    FRAGMENT_SHADER: 0x8B30,
    COMPILE_STATUS: 0x8B81,
    LINK_STATUS: 0x8B82,
    ARRAY_BUFFER: 0x8892,
    STATIC_DRAW: 0x88E4,
    FLOAT: 0x1406,
    createShader: vi.fn(() => {
      if (createShaderResults.length === 0) return { id: 'extra-shader' };
      return createShaderResults.shift() as WebGLShader | null;
    }),
    shaderSource: vi.fn(),
    compileShader: vi.fn(),
    getShaderParameter: vi.fn((shader, param) => {
      if (param !== 0x8B81) return false;
      if (shader === vertexShader) return compileStatuses[0] ?? true;
      if (shader === fragmentShader) return compileStatuses[1] ?? true;
      return true;
    }),
    getShaderInfoLog: vi.fn(() => options.shaderInfoLog ?? 'shader failed'),
    deleteShader: vi.fn(),
    createProgram: vi.fn(() => program),
    attachShader: vi.fn(),
    linkProgram: vi.fn(),
    getProgramParameter: vi.fn((_program, param) => {
      if (param !== 0x8B82) return false;
      return options.programLinkStatus ?? true;
    }),
    getProgramInfoLog: vi.fn(() => options.programInfoLog ?? 'link failed'),
    deleteProgram: vi.fn(),
    useProgram: vi.fn(),
    getUniformLocation: vi.fn((_program, name) => ({ name })),
    createVertexArray: vi.fn(() => ({ id: 'vao' })),
    bindVertexArray: vi.fn(),
    createBuffer: vi.fn(() => ({ id: 'buffer' })),
    bindBuffer: vi.fn(),
    bufferData: vi.fn(),
    getAttribLocation: vi.fn((_program, name) => (name === 'a_position' ? 0 : 1)),
    enableVertexAttribArray: vi.fn(),
    vertexAttribPointer: vi.fn(),
  } as unknown as WebGL2RenderingContext;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('createWebGLTileProgram', () => {
  it('builds the tile program and quad geometry for successful initialization', () => {
    const gl = createProgramGl() as any;

    const result = createWebGLTileProgram(gl);

    expect(result.program).toEqual({ id: 'program' });
    expect(result.uRect).toEqual({ name: 'u_rect' });
    expect(result.uResolution).toEqual({ name: 'u_resolution' });
    expect(result.uMode).toEqual({ name: 'u_mode' });
    expect(result.uColor).toEqual({ name: 'u_color' });
    expect(gl.useProgram).toHaveBeenCalledWith(result.program);
    expect(gl.bufferData).toHaveBeenCalledWith(
      gl.ARRAY_BUFFER,
      expect.any(Float32Array),
      gl.STATIC_DRAW,
    );
    expect(gl.enableVertexAttribArray).toHaveBeenCalledTimes(2);
    expect(gl.vertexAttribPointer).toHaveBeenCalledTimes(2);
    expect(gl.bindVertexArray).toHaveBeenLastCalledWith(null);
  });

  it('throws a shader compile error and deletes the failed shader', () => {
    const gl = createProgramGl({
      compileStatuses: [false, true],
      shaderInfoLog: 'vertex compile failed',
    }) as any;

    expect(() => createWebGLTileProgram(gl)).toThrow('Shader compile error: vertex compile failed');
    expect(gl.deleteShader).toHaveBeenCalledWith({ id: 'vs' });
  });

  it('throws a program link error and cleans up the program and compiled shaders', () => {
    const gl = createProgramGl({
      programLinkStatus: false,
      programInfoLog: 'program link failed',
    }) as any;

    expect(() => createWebGLTileProgram(gl)).toThrow('Program link error: program link failed');
    expect(gl.deleteProgram).toHaveBeenCalledWith({ id: 'program' });
    expect(gl.deleteShader).toHaveBeenCalledWith({ id: 'vs' });
    expect(gl.deleteShader).toHaveBeenCalledWith({ id: 'fs' });
  });

  it('throws when shader creation fails before compilation starts', () => {
    const gl = createProgramGl({
      createShaderResults: [null, { id: 'fs' } as WebGLShader],
    }) as any;

    expect(() => createWebGLTileProgram(gl)).toThrow('Failed to create shader');
  });
});
