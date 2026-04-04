import { vi } from 'vitest';

function createCanvas2DContextMock(canvas: HTMLCanvasElement): CanvasRenderingContext2D {
  return {
    canvas,
    fillStyle: '',
    strokeStyle: '',
    clearRect: vi.fn(),
    drawImage: vi.fn(),
    putImageData: vi.fn(),
    fillRect: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    closePath: vi.fn(),
    fill: vi.fn(),
    stroke: vi.fn(),
  } as unknown as CanvasRenderingContext2D;
}

export function installCanvasGetContextMock() {
  const contexts = new WeakMap<HTMLCanvasElement, CanvasRenderingContext2D>();
  return vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (
    this: HTMLCanvasElement,
    contextId: string,
  ) {
    if (contextId === 'webgl2') return null as unknown as WebGL2RenderingContext;
    if (contextId !== '2d') return null;

    let ctx = contexts.get(this);
    if (!ctx) {
      ctx = createCanvas2DContextMock(this);
      contexts.set(this, ctx);
    }
    return ctx;
  });
}
