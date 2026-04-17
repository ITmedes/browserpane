import { describe, expect, it, vi } from 'vitest';

import { FileDownloadRuntime } from '../file-transfer/download-runtime.js';

describe('FileDownloadRuntime', () => {
  it('assembles a completed download from header, chunk, and complete messages', async () => {
    const runtime = new FileDownloadRuntime();

    expect(runtime.handleMessage({
      type: 'header',
      id: 9,
      filename: 'invoice.pdf',
      size: 11,
      mime: 'application/pdf',
    })).toBeNull();

    expect(runtime.handleMessage({
      type: 'chunk',
      id: 9,
      seq: 0,
      data: new TextEncoder().encode('hello world'),
    })).toBeNull();

    const completedDownload = runtime.handleMessage({
      type: 'complete',
      id: 9,
    });

    expect(completedDownload).toMatchObject({
      filename: 'invoice.pdf',
      mime: 'application/pdf',
      expectedSize: 11,
      receivedSize: 11,
    });
    expect(completedDownload).not.toBeNull();
    expect(await new Blob(completedDownload!.chunks).text()).toBe('hello world');
  });

  it('warns and drops chunks that arrive before a header', () => {
    const warn = vi.fn();
    const runtime = new FileDownloadRuntime(warn);

    expect(runtime.handleMessage({
      type: 'chunk',
      id: 7,
      seq: 0,
      data: new Uint8Array([1, 2, 3]),
    })).toBeNull();

    expect(warn).toHaveBeenCalledWith('[bpane] dropped file chunk without header', {
      id: 7,
      seq: 0,
    });
  });

  it('warns, clears state, and drops completion after a sequence mismatch', () => {
    const warn = vi.fn();
    const runtime = new FileDownloadRuntime(warn);

    runtime.handleMessage({
      type: 'header',
      id: 3,
      filename: 'report.txt',
      size: 8,
      mime: 'text/plain',
    });
    runtime.handleMessage({
      type: 'chunk',
      id: 3,
      seq: 0,
      data: new TextEncoder().encode('part-1'),
    });

    expect(runtime.handleMessage({
      type: 'chunk',
      id: 3,
      seq: 2,
      data: new TextEncoder().encode('part-2'),
    })).toBeNull();
    expect(runtime.handleMessage({
      type: 'complete',
      id: 3,
    })).toBeNull();

    expect(warn.mock.calls).toEqual([
      [
        '[bpane] file chunk sequence mismatch',
        { id: 3, expectedSeq: 1, seq: 2 },
      ],
      [
        '[bpane] dropped file completion without header',
        { id: 3 },
      ],
    ]);
  });

  it('warns on size mismatch but still returns the completed download', async () => {
    const warn = vi.fn();
    const runtime = new FileDownloadRuntime(warn);

    runtime.handleMessage({
      type: 'header',
      id: 5,
      filename: '',
      size: 99,
      mime: '',
    });
    runtime.handleMessage({
      type: 'chunk',
      id: 5,
      seq: 0,
      data: new TextEncoder().encode('hello'),
    });

    const completedDownload = runtime.handleMessage({
      type: 'complete',
      id: 5,
    });

    expect(completedDownload).toMatchObject({
      filename: 'download-5',
      mime: 'application/octet-stream',
      expectedSize: 99,
      receivedSize: 5,
    });
    expect(await new Blob(completedDownload!.chunks).text()).toBe('hello');
    expect(warn).toHaveBeenCalledWith('[bpane] file download size mismatch', {
      id: 5,
      expectedSize: 99,
      receivedSize: 5,
    });
  });
});
