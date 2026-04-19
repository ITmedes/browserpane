import { describe, expect, it, vi } from 'vitest';

import { CH_CONTROL, CH_FILE_DOWN, encodeFrame } from '../protocol.js';
import { SessionStreamReaderRuntime } from '../session-stream-reader-runtime.js';

class MockReadableStream {
  private readonly reader = new MockReader();

  getReader(): MockReader {
    return this.reader;
  }

  pushValue(value: Uint8Array): void {
    this.reader.pushValue(value);
  }

  end(): void {
    this.reader.end();
  }

  fail(error: unknown): void {
    this.reader.fail(error);
  }
}

class MockReader {
  private queue: Array<{ value: Uint8Array | undefined; done: boolean }> = [];
  private resolvers: Array<{
    resolve: (result: { value: Uint8Array | undefined; done: boolean }) => void;
    reject: (error: unknown) => void;
  }> = [];
  private ended = false;
  private failure: unknown = null;

  pushValue(value: Uint8Array): void {
    if (this.resolvers.length > 0) {
      this.resolvers.shift()!.resolve({ value, done: false });
      return;
    }
    this.queue.push({ value, done: false });
  }

  end(): void {
    this.ended = true;
    if (this.resolvers.length > 0) {
      this.resolvers.shift()!.resolve({ value: undefined, done: true });
      return;
    }
    this.queue.push({ value: undefined, done: true });
  }

  fail(error: unknown): void {
    this.failure = error;
    if (this.resolvers.length > 0) {
      this.resolvers.shift()!.reject(error);
    }
  }

  async read(): Promise<{ value: Uint8Array | undefined; done: boolean }> {
    if (this.failure !== null) {
      const error = this.failure;
      this.failure = null;
      throw error;
    }
    if (this.queue.length > 0) {
      return this.queue.shift()!;
    }
    if (this.ended) {
      return { value: undefined, done: true };
    }
    return new Promise((resolve, reject) => {
      this.resolvers.push({ resolve, reject });
    });
  }
}

function createRuntime() {
  let connected = true;
  const onFrame = vi.fn();
  const recordRx = vi.fn();
  const onReadError = vi.fn();
  const runtime = new SessionStreamReaderRuntime({
    isConnected: () => connected,
    onFrame,
    recordRx,
    onReadError,
  });
  const readable = new MockReadableStream();

  return {
    runtime,
    readable,
    onFrame,
    recordRx,
    onReadError,
    setConnected: (value: boolean) => {
      connected = value;
    },
  };
}

describe('SessionStreamReaderRuntime', () => {
  it('buffers partial frames until the full payload arrives', async () => {
    const { runtime, readable, onFrame, recordRx } = createRuntime();
    const controlFrame = encodeFrame(CH_CONTROL, new Uint8Array([0x04, 0x63]));
    const firstHalf = controlFrame.subarray(0, 4);
    const secondHalf = controlFrame.subarray(4);

    const readPromise = runtime.readStream({
      readable: readable as unknown as ReadableStream<Uint8Array>,
    } as WebTransportBidirectionalStream);

    readable.pushValue(firstHalf);
    await Promise.resolve();
    expect(onFrame).not.toHaveBeenCalled();

    readable.pushValue(secondHalf);
    readable.end();
    await readPromise;

    expect(onFrame).toHaveBeenCalledOnce();
    expect(onFrame).toHaveBeenCalledWith(CH_CONTROL, new Uint8Array([0x04, 0x63]));
    expect(recordRx).toHaveBeenCalledWith(CH_CONTROL, controlFrame.length);
  });

  it('parses multiple frames from a single chunk in order', async () => {
    const { runtime, readable, onFrame, recordRx } = createRuntime();
    const firstFrame = encodeFrame(CH_CONTROL, new Uint8Array([0x04]));
    const secondFrame = encodeFrame(CH_FILE_DOWN, new Uint8Array([0x02, 0x03]));
    const combined = new Uint8Array(firstFrame.length + secondFrame.length);
    combined.set(firstFrame, 0);
    combined.set(secondFrame, firstFrame.length);

    const readPromise = runtime.readStream({
      readable: readable as unknown as ReadableStream<Uint8Array>,
    } as WebTransportBidirectionalStream);

    readable.pushValue(combined);
    readable.end();
    await readPromise;

    expect(onFrame).toHaveBeenNthCalledWith(1, CH_CONTROL, new Uint8Array([0x04]));
    expect(onFrame).toHaveBeenNthCalledWith(2, CH_FILE_DOWN, new Uint8Array([0x02, 0x03]));
    expect(recordRx).toHaveBeenNthCalledWith(1, CH_CONTROL, firstFrame.length);
    expect(recordRx).toHaveBeenNthCalledWith(2, CH_FILE_DOWN, secondFrame.length);
  });

  it('reports read errors while connected', async () => {
    const { runtime, readable, onReadError } = createRuntime();
    const error = new Error('stream boom');
    const readPromise = runtime.readStream({
      readable: readable as unknown as ReadableStream<Uint8Array>,
    } as WebTransportBidirectionalStream);

    readable.fail(error);
    await readPromise;

    expect(onReadError).toHaveBeenCalledWith(error);
  });

  it('ignores read errors after disconnect', async () => {
    const { runtime, readable, onReadError, setConnected } = createRuntime();
    const error = new Error('stream boom');
    const readPromise = runtime.readStream({
      readable: readable as unknown as ReadableStream<Uint8Array>,
    } as WebTransportBidirectionalStream);

    setConnected(false);
    readable.fail(error);
    await readPromise;

    expect(onReadError).not.toHaveBeenCalled();
  });
});
