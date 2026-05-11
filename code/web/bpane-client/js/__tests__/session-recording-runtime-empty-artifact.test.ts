import { describe, expect, it, vi } from 'vitest';

import { SessionRecordingRuntime } from '../session-recording-runtime.js';

class MockMediaStream {
  getTracks(): any[] {
    return [{ kind: 'video', stop: vi.fn() }];
  }
}

class MockBlobEvent extends Event {
  readonly data: Blob;

  constructor(data: Blob) {
    super('dataavailable');
    this.data = data;
  }
}

class MockMediaRecorder {
  readonly start = vi.fn();
  readonly stop = vi.fn();
  readonly requestData = vi.fn();
  ondataavailable: ((event: BlobEvent) => void) | null = null;
  onstop: ((event: Event) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;

  emitData(bytes: number[]): void {
    this.ondataavailable?.(new MockBlobEvent(new Blob([new Uint8Array(bytes)])) as unknown as BlobEvent);
  }

  emitStop(): void {
    this.onstop?.(new Event('stop'));
  }
}

describe('SessionRecordingRuntime empty artifact handling', () => {
  it('rejects tiny recorder artifacts instead of returning empty downloads', async () => {
    const recorder = new MockMediaRecorder();
    const runtime = new SessionRecordingRuntime({
      createVideoStream: vi.fn(() => new MockMediaStream() as unknown as MediaStream),
      getAudioStream: vi.fn(async () => null),
      stopVideoStream: vi.fn(),
      mediaRecorderFactory: () => recorder as unknown as MediaRecorder,
      mediaStreamFactory: () => new MockMediaStream() as unknown as MediaStream,
    });

    await runtime.start();
    recorder.emitData([1, 2, 3, 4, 5]);
    const stopPromise = runtime.stop();
    recorder.emitStop();

    await expect(stopPromise).rejects.toThrow('empty artifact (5 bytes)');
    expect(runtime.isRecording()).toBe(false);
  });
});
