import type { TileCommand } from '../tile-cache.js';

export interface TileBatchCommandApplierCallbacks {
  applyScrollCopy(dx: number, dy: number, regionTop: number, regionBottom: number, regionRight: number): void;
  setGridOffset(offsetX: number, offsetY: number): void;
  setApplyOffsetMode(applyOffset: boolean): void;
  setVideoRegion(region: { x: number; y: number; w: number; h: number } | null): void;
  drawFill(col: number, row: number, rgba: number): void;
  drawCacheHit(col: number, row: number, hash: bigint, frameSeq: number): void;
  drawQoi(col: number, row: number, hash: bigint, data: Uint8Array, epoch: number): void;
  drawZstd(col: number, row: number, hash: bigint, data: Uint8Array, epoch: number): void;
}

export class TileBatchCommandApplier {
  private readonly callbacks: TileBatchCommandApplierCallbacks;

  constructor(callbacks: TileBatchCommandApplierCallbacks) {
    this.callbacks = callbacks;
  }

  applyCommands(args: {
    commands: TileCommand[];
    frameSeq: number;
    epoch: number;
    shouldContinue?: () => boolean;
  }): boolean {
    const { commands, frameSeq, epoch, shouldContinue } = args;
    for (const command of commands) {
      if (shouldContinue && !shouldContinue()) {
        return false;
      }
      this.applyCommand(command, frameSeq, epoch);
    }
    return true;
  }

  private applyCommand(command: TileCommand, frameSeq: number, epoch: number): void {
    switch (command.type) {
      case 'scroll-copy':
        this.callbacks.applyScrollCopy(
          command.dx,
          command.dy,
          command.regionTop,
          command.regionBottom,
          command.regionRight,
        );
        break;

      case 'grid-offset':
        this.callbacks.setGridOffset(command.offsetX, command.offsetY);
        break;

      case 'tile-draw-mode':
        this.callbacks.setApplyOffsetMode(command.applyOffset);
        break;

      case 'fill':
        this.callbacks.drawFill(command.col, command.row, command.rgba);
        break;

      case 'cache-hit':
        this.callbacks.drawCacheHit(command.col, command.row, command.hash, frameSeq);
        break;

      case 'qoi':
        this.callbacks.drawQoi(command.col, command.row, command.hash, command.data, epoch);
        break;

      case 'zstd':
        this.callbacks.drawZstd(command.col, command.row, command.hash, command.data, epoch);
        break;

      case 'video-region':
        this.callbacks.setVideoRegion(
          command.w > 0 && command.h > 0
            ? { x: command.x, y: command.y, w: command.w, h: command.h }
            : null,
        );
        break;

      case 'grid-config':
      case 'batch-end':
      case 'scroll-stats':
        break;
    }
  }
}
