import type { TileCommand } from '../tile-cache.js';

export interface QueuedTileBatch {
  frameSeq: number;
  commands: TileCommand[];
  epoch: number;
}

export class TileBatchSequencer {
  private flushChain: Promise<void> = Promise.resolve();
  private lastAppliedFrameSeq: number | null = null;
  private epoch = 0;

  invalidate(): void {
    this.epoch++;
    this.lastAppliedFrameSeq = null;
  }

  reset(): void {
    this.invalidate();
    this.flushChain = Promise.resolve();
  }

  isCurrentEpoch(epoch: number): boolean {
    return epoch === this.epoch;
  }

  enqueueBatch(
    frameSeq: number,
    commands: TileCommand[],
    applyBatch: (batch: QueuedTileBatch) => Promise<boolean> | boolean,
  ): void {
    const batch: QueuedTileBatch = {
      frameSeq,
      commands: [...commands],
      epoch: this.epoch,
    };

    this.flushChain = this.flushChain.then(async () => {
      if (!this.canStartBatch(batch.frameSeq, batch.epoch)) {
        return;
      }

      const completed = await applyBatch(batch);
      if (!completed) {
        return;
      }

      if (batch.epoch === this.epoch) {
        this.lastAppliedFrameSeq = batch.frameSeq;
      }
    });
  }

  flush(): Promise<void> {
    return this.flushChain;
  }

  private canStartBatch(frameSeq: number, epoch: number): boolean {
    return epoch === this.epoch && this.isNewerFrameSeq(frameSeq);
  }

  private isNewerFrameSeq(seq: number): boolean {
    if (this.lastAppliedFrameSeq === null) {
      return true;
    }
    const diff = (seq - this.lastAppliedFrameSeq) >>> 0;
    return diff !== 0 && diff < 0x80000000;
  }
}
