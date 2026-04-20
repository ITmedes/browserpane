import type { DecodedFileMessage } from './codec.js';

type WarnFn = (message: string, details: Record<string, number>) => void;

interface DownloadState {
  filename: string;
  mime: string;
  expectedSize: number;
  receivedSize: number;
  expectedSeq: number;
  chunks: ArrayBuffer[];
}

export interface CompletedDownload {
  filename: string;
  mime: string;
  expectedSize: number;
  receivedSize: number;
  chunks: ArrayBuffer[];
}

export class FileDownloadRuntime {
  private readonly warn: WarnFn;
  private readonly activeDownloads = new Map<number, DownloadState>();

  constructor(warn: WarnFn = (message, details) => console.warn(message, details)) {
    this.warn = warn;
  }

  destroy(): void {
    this.activeDownloads.clear();
  }

  handleMessage(message: DecodedFileMessage): CompletedDownload | null {
    switch (message.type) {
      case 'header':
        this.activeDownloads.set(message.id, {
          filename: message.filename || `download-${message.id}`,
          mime: message.mime || 'application/octet-stream',
          expectedSize: message.size,
          receivedSize: 0,
          expectedSeq: 0,
          chunks: [],
        });
        return null;
      case 'chunk':
        return this.handleChunk(message.id, message.seq, message.data);
      case 'complete':
        return this.handleComplete(message.id);
    }
  }

  private handleChunk(id: number, seq: number, data: Uint8Array): null {
    const download = this.activeDownloads.get(id);
    if (!download) {
      this.warn('[bpane] dropped file chunk without header', { id, seq });
      return null;
    }

    if (seq !== download.expectedSeq) {
      this.warn('[bpane] file chunk sequence mismatch', {
        id,
        expectedSeq: download.expectedSeq,
        seq,
      });
      this.activeDownloads.delete(id);
      return null;
    }

    download.chunks.push(new Uint8Array(data).buffer);
    download.receivedSize += data.byteLength;
    download.expectedSeq += 1;
    return null;
  }

  private handleComplete(id: number): CompletedDownload | null {
    const download = this.activeDownloads.get(id);
    if (!download) {
      this.warn('[bpane] dropped file completion without header', { id });
      return null;
    }

    this.activeDownloads.delete(id);

    if (download.expectedSize > 0 && download.receivedSize !== download.expectedSize) {
      this.warn('[bpane] file download size mismatch', {
        id,
        expectedSize: download.expectedSize,
        receivedSize: download.receivedSize,
      });
    }

    return {
      filename: download.filename,
      mime: download.mime,
      expectedSize: download.expectedSize,
      receivedSize: download.receivedSize,
      chunks: download.chunks,
    };
  }
}
