export interface CursorShapeInput {
  width: number;
  height: number;
  hotspotX: number;
  hotspotY: number;
  data: Uint8Array;
}

export interface SessionCursorRuntimeInput {
  canvas: HTMLCanvasElement;
  cursorEl: HTMLCanvasElement | null;
  cursorCtx: CanvasRenderingContext2D | null;
  createImageData?: (data: Uint8ClampedArray, width: number, height: number) => ImageData;
}

export class SessionCursorRuntime {
  private readonly canvas: HTMLCanvasElement;
  private readonly cursorEl: HTMLCanvasElement | null;
  private readonly cursorCtx: CanvasRenderingContext2D | null;
  private readonly createImageData: (data: Uint8ClampedArray, width: number, height: number) => ImageData;

  private cursorHotspot: { x: number; y: number } = { x: 0, y: 0 };
  private cursorPosition: { x: number; y: number } | null = null;
  private cursorBitmap: HTMLCanvasElement | null = null;

  constructor(input: SessionCursorRuntimeInput) {
    this.canvas = input.canvas;
    this.cursorEl = input.cursorEl;
    this.cursorCtx = input.cursorCtx;
    this.createImageData = input.createImageData
      ?? ((data, width, height) => new ImageData(Uint8ClampedArray.from(data), width, height));
  }

  handlePayload(payload: Uint8Array): boolean {
    if (!payload.length) {
      return false;
    }
    const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    const tag = view.getUint8(0);
    if (tag === 0x01 && payload.length >= 5) {
      const x = view.getUint16(1, true);
      const y = view.getUint16(3, true);
      this.drawMove(x, y);
      return true;
    }
    if (tag === 0x02 && payload.length >= 11) {
      const width = view.getUint16(1, true);
      const height = view.getUint16(3, true);
      const hotspotX = view.getUint8(5);
      const hotspotY = view.getUint8(6);
      const dataLen = view.getUint32(7, true);
      if (payload.length < 11 + dataLen) {
        return false;
      }
      this.drawShape({
        width,
        height,
        hotspotX,
        hotspotY,
        data: payload.subarray(11, 11 + dataLen),
      });
      return true;
    }
    return false;
  }

  drawMove(x: number, y: number): void {
    this.drawCursor(null, x, y);
  }

  drawShape(input: CursorShapeInput): void {
    this.cursorHotspot = {
      x: input.hotspotX,
      y: input.hotspotY,
    };
    this.drawCursor({
      width: input.width,
      height: input.height,
      data: input.data,
    }, this.cursorPosition?.x ?? null, this.cursorPosition?.y ?? null);
  }

  reset(): void {
    this.cursorHotspot = { x: 0, y: 0 };
    this.cursorPosition = null;
    this.cursorBitmap = null;
    if (this.cursorEl && this.cursorCtx) {
      this.cursorCtx.clearRect(0, 0, this.cursorEl.width, this.cursorEl.height);
    }
  }

  private drawCursor(
    shape: { width: number; height: number; data: Uint8Array } | null,
    moveX: number | null,
    moveY: number | null,
  ): void {
    if (!this.cursorEl || !this.cursorCtx) {
      return;
    }
    if (moveX !== null && moveY !== null) {
      this.cursorPosition = { x: moveX, y: moveY };
    }
    const w = Math.max(64, this.canvas.width);
    const h = Math.max(64, this.canvas.height);
    if (this.cursorEl.width !== w || this.cursorEl.height !== h) {
      this.cursorEl.width = w;
      this.cursorEl.height = h;
    }
    this.cursorCtx.clearRect(0, 0, this.cursorEl.width, this.cursorEl.height);

    if (shape) {
      if (shape.data.length === shape.width * shape.height * 4) {
        const imageData = this.createImageData(
          new Uint8ClampedArray(shape.data),
          shape.width,
          shape.height,
        );
        const off = document.createElement('canvas');
        off.width = shape.width;
        off.height = shape.height;
        const offCtx = off.getContext('2d');
        if (offCtx) {
          offCtx.putImageData(imageData, 0, 0);
          this.cursorBitmap = off;
        }
      } else {
        this.cursorBitmap = null;
      }
    }

    if (!this.cursorPosition) {
      return;
    }

    const x = this.cursorPosition.x - this.cursorHotspot.x;
    const y = this.cursorPosition.y - this.cursorHotspot.y;
    if (this.cursorBitmap) {
      this.cursorCtx.drawImage(this.cursorBitmap, x, y);
      return;
    }

    this.cursorCtx.fillStyle = '#ffffff';
    this.cursorCtx.strokeStyle = '#000000';
    this.cursorCtx.beginPath();
    this.cursorCtx.moveTo(x, y);
    this.cursorCtx.lineTo(x + 12, y + 6);
    this.cursorCtx.lineTo(x + 6, y + 12);
    this.cursorCtx.closePath();
    this.cursorCtx.fill();
    this.cursorCtx.stroke();
  }
}
