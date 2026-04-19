import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installCanvasGetContextMock } from './canvas-test-helpers.js';
import { SessionCursorRuntime } from '../session-cursor-runtime.js';

function createRuntime() {
  const canvas = document.createElement('canvas');
  canvas.width = 800;
  canvas.height = 600;
  const cursorEl = document.createElement('canvas');
  cursorEl.width = 800;
  cursorEl.height = 600;
  const cursorCtx = cursorEl.getContext('2d') as CanvasRenderingContext2D;

  const runtime = new SessionCursorRuntime({
    canvas,
    cursorEl,
    cursorCtx,
    createImageData: (data, width, height) => ({
      data,
      width,
      height,
    } as unknown as ImageData),
  });

  return {
    runtime,
    canvas,
    cursorEl,
    cursorCtx: cursorCtx as unknown as {
      clearRect: ReturnType<typeof vi.fn>;
      drawImage: ReturnType<typeof vi.fn>;
      beginPath: ReturnType<typeof vi.fn>;
      moveTo: ReturnType<typeof vi.fn>;
      lineTo: ReturnType<typeof vi.fn>;
      closePath: ReturnType<typeof vi.fn>;
      fill: ReturnType<typeof vi.fn>;
      stroke: ReturnType<typeof vi.fn>;
      fillStyle: string;
      strokeStyle: string;
    },
  };
}

beforeEach(() => {
  installCanvasGetContextMock();
});

describe('SessionCursorRuntime', () => {
  it('draws the fallback pointer on cursor move updates', () => {
    const {
      runtime,
      cursorCtx,
    } = createRuntime();

    const payload = new Uint8Array(5);
    const view = new DataView(payload.buffer);
    view.setUint8(0, 0x01);
    view.setUint16(1, 120, true);
    view.setUint16(3, 240, true);

    expect(runtime.handlePayload(payload)).toBe(true);
    expect(cursorCtx.clearRect).toHaveBeenCalled();
    expect(cursorCtx.beginPath).toHaveBeenCalledOnce();
    expect(cursorCtx.moveTo).toHaveBeenCalledWith(120, 240);
  });

  it('reuses the last cursor position for shape-only updates', () => {
    const {
      runtime,
      cursorCtx,
    } = createRuntime();

    const movePayload = new Uint8Array(5);
    const moveView = new DataView(movePayload.buffer);
    moveView.setUint8(0, 0x01);
    moveView.setUint16(1, 100, true);
    moveView.setUint16(3, 200, true);
    runtime.handlePayload(movePayload);

    cursorCtx.drawImage.mockClear();

    const width = 16;
    const height = 16;
    const dataLen = width * height * 4;
    const shapePayload = new Uint8Array(11 + dataLen);
    const shapeView = new DataView(shapePayload.buffer);
    shapeView.setUint8(0, 0x02);
    shapeView.setUint16(1, width, true);
    shapeView.setUint16(3, height, true);
    shapeView.setUint8(5, 8);
    shapeView.setUint8(6, 8);
    shapeView.setUint32(7, dataLen, true);
    for (let i = 0; i < dataLen; i += 4) {
      shapePayload[11 + i] = 255;
      shapePayload[11 + i + 1] = 255;
      shapePayload[11 + i + 2] = 255;
      shapePayload[11 + i + 3] = 255;
    }

    expect(runtime.handlePayload(shapePayload)).toBe(true);
    expect(cursorCtx.drawImage).toHaveBeenCalledTimes(1);
    expect(cursorCtx.drawImage.mock.calls[0]?.[1]).toBe(92);
    expect(cursorCtx.drawImage.mock.calls[0]?.[2]).toBe(192);
  });

  it('ignores malformed cursor payloads', () => {
    const {
      runtime,
      cursorCtx,
    } = createRuntime();

    expect(runtime.handlePayload(new Uint8Array())).toBe(false);
    expect(runtime.handlePayload(new Uint8Array([0x01, 0x00]))).toBe(false);
    expect(runtime.handlePayload(new Uint8Array([0x02, 0x00]))).toBe(false);
    expect(cursorCtx.clearRect).not.toHaveBeenCalled();
  });

  it('resets overlay state and clears the cursor canvas', () => {
    const {
      runtime,
      cursorCtx,
      cursorEl,
    } = createRuntime();

    runtime.drawMove(20, 30);
    cursorCtx.clearRect.mockClear();

    runtime.reset();

    expect(cursorCtx.clearRect).toHaveBeenCalledWith(0, 0, cursorEl.width, cursorEl.height);
    cursorCtx.beginPath.mockClear();
    runtime.drawShape({
      width: 1,
      height: 1,
      hotspotX: 0,
      hotspotY: 0,
      data: new Uint8Array([255, 255, 255, 255]),
    });
    expect(cursorCtx.beginPath).not.toHaveBeenCalled();
    expect(cursorCtx.drawImage).not.toHaveBeenCalled();
  });
});
