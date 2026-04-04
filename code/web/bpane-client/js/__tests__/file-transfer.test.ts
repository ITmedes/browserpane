import { afterEach, describe, expect, it, vi } from 'vitest';

import { CH_FILE_UP } from '../protocol.js';
import {
  FileTransferController,
  decodeFileMessage,
  encodeFileChunk,
  encodeFileComplete,
  encodeFileHeader,
} from '../file-transfer.js';

describe('FileTransferController', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('uploads files as header, chunk, and completion frames', async () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const sent: Array<{ channelId: number; payload: Uint8Array }> = [];
    const controller = new FileTransferController({
      container,
      enabled: true,
      sendFrame: (channelId, payload) => sent.push({ channelId, payload }),
    });

    const file = new File(['hello world'], 'report.txt', { type: 'text/plain' });
    await controller.uploadFiles([file]);

    expect(sent).toHaveLength(3);
    expect(sent.every((frame) => frame.channelId === CH_FILE_UP)).toBe(true);

    const header = decodeFileMessage(sent[0].payload);
    expect(header).toMatchObject({
      type: 'header',
      filename: 'report.txt',
      size: 11,
      mime: 'text/plain',
    });

    const chunk = decodeFileMessage(sent[1].payload);
    expect(chunk.type).toBe('chunk');
    if (chunk.type === 'chunk') {
      expect(new TextDecoder().decode(chunk.data)).toBe('hello world');
      expect(chunk.seq).toBe(0);
    }

    const complete = decodeFileMessage(sent[2].payload);
    expect(complete).toMatchObject({ type: 'complete' });

    controller.destroy();
  });

  it('reconstructs file downloads and triggers a local browser download', async () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const controller = new FileTransferController({
      container,
      enabled: true,
      sendFrame: vi.fn(),
    });

    let downloadedName = '';
    let capturedBlob: Blob | null = null;
    const createObjectURL = vi.fn((blob: Blob | MediaSource) => {
      if (blob instanceof Blob) capturedBlob = blob;
      return 'blob:test';
    });
    const revokeObjectURL = vi.fn();
    Object.defineProperty(URL, 'createObjectURL', {
      configurable: true,
      value: createObjectURL,
    });
    Object.defineProperty(URL, 'revokeObjectURL', {
      configurable: true,
      value: revokeObjectURL,
    });
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function click(this: HTMLAnchorElement) {
      downloadedName = this.download;
    });

    controller.handleFrame(
      encodeFileHeader({
        id: 9,
        filename: 'invoice.pdf',
        size: 11,
        mime: 'application/pdf',
      }),
    );
    controller.handleFrame(
      encodeFileChunk({
        id: 9,
        seq: 0,
        data: new TextEncoder().encode('hello world'),
      }),
    );
    controller.handleFrame(encodeFileComplete(9));

    expect(downloadedName).toBe('invoice.pdf');
    expect(capturedBlob).not.toBeNull();
    expect(await capturedBlob!.text()).toBe('hello world');

    controller.destroy();
  });
});
