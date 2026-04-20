import { describe, expect, it, vi } from 'vitest';

import { FileUploadRuntime } from '../file-transfer/upload-runtime.js';
import { decodeFileMessage } from '../file-transfer.js';
import { CH_FILE_UP } from '../protocol.js';

describe('FileUploadRuntime', () => {
  it('uploads a file as header, chunk, and completion frames', async () => {
    const sent: Array<{ channelId: number; payload: Uint8Array }> = [];
    const runtime = new FileUploadRuntime((channelId, payload) => sent.push({ channelId, payload }));

    await runtime.uploadFiles([
      new File(['hello world'], 'report.txt', { type: 'text/plain' }),
    ]);

    expect(sent).toHaveLength(3);
    expect(sent.every((frame) => frame.channelId === CH_FILE_UP)).toBe(true);
    expect(decodeFileMessage(sent[0].payload)).toMatchObject({
      type: 'header',
      id: 1,
      filename: 'report.txt',
      size: 11,
      mime: 'text/plain',
    });
    const chunk = decodeFileMessage(sent[1].payload);
    expect(chunk).toMatchObject({
      type: 'chunk',
      id: 1,
      seq: 0,
    });
    expect(chunk.type === 'chunk' ? new TextDecoder().decode(chunk.data) : '').toBe('hello world');
    expect(decodeFileMessage(sent[2].payload)).toMatchObject({
      type: 'complete',
      id: 1,
    });
  });

  it('falls back to generated filename and octet-stream mime when metadata is missing', async () => {
    const sent: Uint8Array[] = [];
    const runtime = new FileUploadRuntime((_channelId, payload) => sent.push(payload));

    await runtime.uploadFiles([
      new File(['abc'], '', { type: '' }),
    ]);

    expect(decodeFileMessage(sent[0])).toMatchObject({
      type: 'header',
      id: 1,
      filename: 'upload-1',
      size: 3,
      mime: 'application/octet-stream',
    });
  });

  it('increments transfer ids across multiple files and emits empty-file completions', async () => {
    const sent: Uint8Array[] = [];
    const runtime = new FileUploadRuntime((_channelId, payload) => sent.push(payload));

    await runtime.uploadFiles([
      new File([''], 'empty.txt', { type: 'text/plain' }),
      new File(['x'], 'next.txt', { type: 'text/plain' }),
    ]);

    expect(decodeFileMessage(sent[0])).toMatchObject({
      type: 'header',
      id: 1,
      filename: 'empty.txt',
      size: 0,
    });
    expect(decodeFileMessage(sent[1])).toMatchObject({
      type: 'complete',
      id: 1,
    });
    expect(decodeFileMessage(sent[2])).toMatchObject({
      type: 'header',
      id: 2,
      filename: 'next.txt',
      size: 1,
    });
  });

  it('splits large files into sequential chunks', async () => {
    const sent: Uint8Array[] = [];
    const runtime = new FileUploadRuntime((_channelId, payload) => sent.push(payload));

    const largePayload = 'a'.repeat((64 * 1024) + 5);
    await runtime.uploadFiles([
      new File([largePayload], 'large.bin', { type: 'application/octet-stream' }),
    ]);

    expect(decodeFileMessage(sent[1])).toMatchObject({
      type: 'chunk',
      id: 1,
      seq: 0,
    });
    expect(decodeFileMessage(sent[2])).toMatchObject({
      type: 'chunk',
      id: 1,
      seq: 1,
    });
    expect(decodeFileMessage(sent[3])).toMatchObject({
      type: 'complete',
      id: 1,
    });

    const firstChunk = decodeFileMessage(sent[1]);
    const secondChunk = decodeFileMessage(sent[2]);
    expect(firstChunk.type === 'chunk' ? firstChunk.data.byteLength : -1).toBe(64 * 1024);
    expect(secondChunk.type === 'chunk' ? secondChunk.data.byteLength : -1).toBe(5);
  });
});
