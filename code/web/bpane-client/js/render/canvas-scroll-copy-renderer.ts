export type CanvasScrollCopyRequest = {
  ctx: CanvasRenderingContext2D;
  dx: number;
  dy: number;
  regionTop: number;
  regionBottom: number;
  regionRight: number;
  screenW: number;
  screenH: number;
};

type ScratchCanvasFactory = () => HTMLCanvasElement;

export class CanvasScrollCopyRenderer {
  private scratchCanvas: HTMLCanvasElement | null = null;
  private scratchContext: CanvasRenderingContext2D | null = null;
  private readonly scratchCanvasFactory: ScratchCanvasFactory;

  constructor(scratchCanvasFactory: ScratchCanvasFactory = () => document.createElement('canvas')) {
    this.scratchCanvasFactory = scratchCanvasFactory;
  }

  reset(): void {
    this.scratchCanvas = null;
    this.scratchContext = null;
  }

  apply(request: CanvasScrollCopyRequest): boolean {
    const {
      ctx,
      dx,
      dy,
      regionTop,
      regionBottom,
      regionRight,
      screenW,
      screenH,
    } = request;
    const canvas = ctx.canvas as HTMLCanvasElement;
    const tx = -dx || 0;
    const ty = -dy || 0;
    const hasRegion = regionTop !== 0 || regionBottom !== screenH || regionRight !== screenW;

    if (hasRegion) {
      const regionWidth = regionRight;
      const regionHeight = regionBottom - regionTop;
      if (regionWidth <= 0 || regionHeight <= 0) {
        return false;
      }

      if (this.ensureScratchCanvas(canvas.width, canvas.height) && this.scratchCanvas && this.scratchContext) {
        this.scratchContext.clearRect(0, 0, canvas.width, canvas.height);
        this.scratchContext.drawImage(
          canvas,
          0, regionTop, regionWidth, regionHeight,
          0, 0, regionWidth, regionHeight,
        );

        const sourceX = Math.max(0, -tx);
        const sourceY = Math.max(0, -ty);
        const destinationX = Math.max(0, tx);
        const destinationY = regionTop + Math.max(0, ty);
        const sourceWidth = regionWidth - Math.abs(tx);
        const sourceHeight = regionHeight - Math.abs(ty);

        if (sourceWidth > 0 && sourceHeight > 0) {
          ctx.drawImage(
            this.scratchCanvas,
            sourceX, sourceY, sourceWidth, sourceHeight,
            destinationX, destinationY, sourceWidth, sourceHeight,
          );
        }
      }

      return true;
    }

    if (this.ensureScratchCanvas(canvas.width, canvas.height) && this.scratchCanvas && this.scratchContext) {
      this.scratchContext.clearRect(0, 0, canvas.width, canvas.height);
      this.scratchContext.drawImage(canvas, 0, 0);
      ctx.drawImage(this.scratchCanvas, tx, ty);
      return true;
    }

    ctx.drawImage(canvas, tx, ty);
    return true;
  }

  private ensureScratchCanvas(width: number, height: number): boolean {
    if (!this.scratchCanvas || this.scratchCanvas.width !== width || this.scratchCanvas.height !== height) {
      const scratchCanvas = this.scratchCanvasFactory();
      scratchCanvas.width = width;
      scratchCanvas.height = height;
      const scratchContext = scratchCanvas.getContext('2d');

      if (!scratchContext) {
        this.scratchCanvas = null;
        this.scratchContext = null;
        return false;
      }

      this.scratchCanvas = scratchCanvas;
      this.scratchContext = scratchContext;
    }

    return this.scratchCanvas !== null && this.scratchContext !== null;
  }
}
