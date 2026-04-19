import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  CH_AUDIO_IN,
  CH_CLIPBOARD,
  CH_CONTROL,
  CH_FILE_UP,
  CH_INPUT,
  CH_VIDEO_IN,
  CTRL_KEYBOARD_LAYOUT,
  encodeFrame,
  INPUT_KEY_EVENT_EX,
  INPUT_MOUSE_MOVE,
} from '../protocol.js';
import { SessionSendRuntime } from '../session-send-runtime.js';

class MockWriter {
  desiredSize: number | null = 1;
  writes: Uint8Array[] = [];

  async write(data: Uint8Array): Promise<void> {
    this.writes.push(new Uint8Array(data));
  }
}

function createRuntime(options: {
  viewerRestricted?: boolean;
} = {}) {
  let viewerRestricted = options.viewerRestricted ?? false;
  const recordTx = vi.fn();
  const onWriteError = vi.fn();
  const writer = new MockWriter();
  const runtime = new SessionSendRuntime({
    isViewerRestricted: () => viewerRestricted,
    recordTx,
    onWriteError,
    setTimeoutFn: window.setTimeout.bind(window),
    clearTimeoutFn: window.clearTimeout.bind(window),
  });

  return {
    runtime,
    writer,
    recordTx,
    onWriteError,
    setViewerRestricted: (value: boolean) => {
      viewerRestricted = value;
    },
  };
}

describe('SessionSendRuntime', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('queues frames before a writer is attached and flushes them in order', () => {
    const {
      runtime,
      writer,
      recordTx,
    } = createRuntime();
    const onAttached = vi.fn();
    const pingPayload = new Uint8Array([0x04]);
    const tilePayload = new Uint8Array([0x09, 0x01]);

    runtime.sendFrame(CH_CONTROL, pingPayload);
    runtime.sendFrame(CH_FILE_UP, tilePayload);
    runtime.attachWriter(writer as unknown as WritableStreamDefaultWriter<Uint8Array>, onAttached);

    expect(writer.writes).toEqual([
      encodeFrame(CH_CONTROL, pingPayload),
      encodeFrame(CH_FILE_UP, tilePayload),
    ]);
    expect(recordTx).toHaveBeenNthCalledWith(1, CH_CONTROL, encodeFrame(CH_CONTROL, pingPayload).length);
    expect(recordTx).toHaveBeenNthCalledWith(2, CH_FILE_UP, encodeFrame(CH_FILE_UP, tilePayload).length);
    expect(onAttached).toHaveBeenCalledOnce();
  });

  it('blocks viewer-only channels but still allows non-layout control messages', () => {
    const {
      runtime,
      writer,
      setViewerRestricted,
    } = createRuntime();
    runtime.attachWriter(writer as unknown as WritableStreamDefaultWriter<Uint8Array>);
    setViewerRestricted(true);

    runtime.sendFrame(CH_INPUT, new Uint8Array([INPUT_MOUSE_MOVE, 0, 0, 0, 0]));
    runtime.sendFrame(CH_CLIPBOARD, new Uint8Array([0x01]));
    runtime.sendFrame(CH_AUDIO_IN, new Uint8Array([0x01]));
    runtime.sendFrame(CH_VIDEO_IN, new Uint8Array([0x01]));
    runtime.sendFrame(CH_FILE_UP, new Uint8Array([0x01]));
    runtime.sendFrame(CH_CONTROL, new Uint8Array([CTRL_KEYBOARD_LAYOUT]));
    runtime.sendFrame(CH_CONTROL, new Uint8Array([0x04]));

    expect(writer.writes).toEqual([
      encodeFrame(CH_CONTROL, new Uint8Array([0x04])),
    ]);
  });

  it('drops mouse-move frames under backpressure but keeps keyboard input', () => {
    const {
      runtime,
      writer,
      recordTx,
    } = createRuntime();
    runtime.attachWriter(writer as unknown as WritableStreamDefaultWriter<Uint8Array>);
    writer.desiredSize = 0;

    const mousePayload = new Uint8Array([INPUT_MOUSE_MOVE, 0x10, 0x00, 0x20, 0x00]);
    const keyPayload = new Uint8Array(11);
    keyPayload[0] = INPUT_KEY_EVENT_EX;
    keyPayload[1] = 18;
    keyPayload[5] = 0;
    keyPayload[7] = 0xe9;

    runtime.sendFrame(CH_INPUT, mousePayload);
    runtime.sendFrame(CH_INPUT, keyPayload);

    expect(writer.writes).toEqual([
      encodeFrame(CH_INPUT, keyPayload),
    ]);
    expect(recordTx).toHaveBeenCalledTimes(1);
  });

  it('keeps only the latest pending camera frame under backpressure and flushes it later', async () => {
    vi.useFakeTimers();
    const {
      runtime,
      writer,
      recordTx,
    } = createRuntime();
    runtime.attachWriter(writer as unknown as WritableStreamDefaultWriter<Uint8Array>);
    writer.desiredSize = 0;

    expect(runtime.sendCameraFrame(new Uint8Array([0x01]))).toBe('queued');
    expect(runtime.sendCameraFrame(new Uint8Array([0x02]))).toBe('replaced');
    expect(writer.writes).toHaveLength(0);

    writer.desiredSize = 1;
    await vi.advanceTimersByTimeAsync(60);

    expect(writer.writes).toEqual([
      encodeFrame(CH_VIDEO_IN, new Uint8Array([0x02])),
    ]);
    expect(recordTx).toHaveBeenCalledWith(CH_VIDEO_IN, encodeFrame(CH_VIDEO_IN, new Uint8Array([0x02])).length);
  });

  it('sends zero-length camera stop frames immediately and clears queued camera state', () => {
    const {
      runtime,
      writer,
      recordTx,
    } = createRuntime();
    runtime.attachWriter(writer as unknown as WritableStreamDefaultWriter<Uint8Array>);
    writer.desiredSize = 0;

    expect(runtime.sendCameraFrame(new Uint8Array([0x01]))).toBe('queued');
    expect(runtime.sendCameraFrame(new Uint8Array())).toBe('sent');

    expect(writer.writes).toEqual([
      encodeFrame(CH_VIDEO_IN, new Uint8Array()),
    ]);
    expect(recordTx).toHaveBeenCalledWith(CH_VIDEO_IN, encodeFrame(CH_VIDEO_IN, new Uint8Array()).length);
  });
});
