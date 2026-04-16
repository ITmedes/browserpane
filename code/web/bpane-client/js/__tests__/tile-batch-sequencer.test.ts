import { describe, expect, it, vi } from 'vitest';

import type { TileCommand } from '../tile-cache.js';
import { TileBatchSequencer } from '../render/tile-batch-sequencer.js';

function fillCommand(col: number): TileCommand {
  return {
    type: 'fill',
    col,
    row: 0,
    rgba: 0xff,
  };
}

function createDeferred(): { promise: Promise<void>; resolve: () => void } {
  let resolve!: () => void;
  const promise = new Promise<void>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

describe('TileBatchSequencer', () => {
  it('serializes queued batches and snapshots the command list', async () => {
    const sequencer = new TileBatchSequencer();
    const started: number[] = [];
    const firstBatch = createDeferred();

    const firstCommands = [fillCommand(1)];
    sequencer.enqueueBatch(1, firstCommands, async ({ frameSeq, commands }) => {
      started.push(frameSeq);
      await firstBatch.promise;
      expect(commands).toHaveLength(1);
      expect(commands[0]).toEqual(fillCommand(1));
      return true;
    });

    firstCommands.push(fillCommand(2));
    sequencer.enqueueBatch(2, [fillCommand(3)], ({ frameSeq }) => {
      started.push(frameSeq);
      return true;
    });

    await Promise.resolve();
    expect(started).toEqual([1]);

    firstBatch.resolve();
    await sequencer.flush();

    expect(started).toEqual([1, 2]);
  });

  it('drops queued stale batches after invalidation', async () => {
    const sequencer = new TileBatchSequencer();
    const appliedFrames: number[] = [];
    const firstBatch = createDeferred();

    sequencer.enqueueBatch(10, [fillCommand(0)], async ({ frameSeq }) => {
      appliedFrames.push(frameSeq);
      await firstBatch.promise;
      return true;
    });
    sequencer.enqueueBatch(11, [fillCommand(1)], ({ frameSeq }) => {
      appliedFrames.push(frameSeq);
      return true;
    });

    await Promise.resolve();
    sequencer.invalidate();
    firstBatch.resolve();
    await sequencer.flush();

    expect(appliedFrames).toEqual([10]);
  });

  it('drops out-of-order older frame sequences but accepts wraparound', async () => {
    const sequencer = new TileBatchSequencer();
    const appliedFrames: number[] = [];

    sequencer.enqueueBatch(2, [fillCommand(0)], ({ frameSeq }) => {
      appliedFrames.push(frameSeq);
      return true;
    });
    sequencer.enqueueBatch(1, [fillCommand(1)], ({ frameSeq }) => {
      appliedFrames.push(frameSeq);
      return true;
    });

    await sequencer.flush();
    expect(appliedFrames).toEqual([2]);

    const wraparoundSequencer = new TileBatchSequencer();
    const wrappedFrames: number[] = [];

    wraparoundSequencer.enqueueBatch(0xffffffff, [fillCommand(0)], ({ frameSeq }) => {
      wrappedFrames.push(frameSeq);
      return true;
    });
    wraparoundSequencer.enqueueBatch(0, [fillCommand(1)], ({ frameSeq }) => {
      wrappedFrames.push(frameSeq);
      return true;
    });

    await wraparoundSequencer.flush();
    expect(wrappedFrames).toEqual([0xffffffff, 0]);
  });

  it('lets batches restart from a fresh sequence after reset', async () => {
    const sequencer = new TileBatchSequencer();
    const appliedFrames: number[] = [];

    sequencer.enqueueBatch(5, [fillCommand(0)], ({ frameSeq }) => {
      appliedFrames.push(frameSeq);
      return true;
    });
    await sequencer.flush();

    sequencer.reset();

    sequencer.enqueueBatch(1, [fillCommand(1)], ({ frameSeq }) => {
      appliedFrames.push(frameSeq);
      return true;
    });
    await sequencer.flush();

    expect(appliedFrames).toEqual([5, 1]);
  });
});
