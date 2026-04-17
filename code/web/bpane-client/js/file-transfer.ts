import { CH_FILE_DOWN, CH_FILE_UP } from './protocol.js';
import { FileTransferCodec, type DecodedFileMessage } from './file-transfer/codec.js';
import {
  FileDownloadRuntime,
  type CompletedDownload,
} from './file-transfer/download-runtime.js';
import { FileTransferDomBindings } from './file-transfer/dom-bindings.js';
import { FileUploadRuntime } from './file-transfer/upload-runtime.js';

type SendFrameFn = (channelId: number, payload: Uint8Array) => void;

interface FileTransferOptions {
  container: HTMLElement;
  enabled: boolean;
  sendFrame: SendFrameFn;
}

export class FileTransferController {
  private enabled: boolean;
  private readonly downloadRuntime = new FileDownloadRuntime();
  private readonly domBindings: FileTransferDomBindings;
  private readonly uploadRuntime: FileUploadRuntime;

  constructor(options: FileTransferOptions) {
    this.enabled = options.enabled;
    this.uploadRuntime = new FileUploadRuntime(options.sendFrame);
    this.domBindings = new FileTransferDomBindings({
      container: options.container,
      enabled: options.enabled,
      onFilesSelected: (files) => {
        void this.uploadFiles(files);
      },
    });
  }

  destroy(): void {
    this.domBindings.destroy();
    this.downloadRuntime.destroy();
  }

  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
    this.domBindings.setEnabled(enabled);
  }

  promptUpload(): void {
    this.domBindings.promptUpload();
  }

  async uploadFiles(filesInput: FileList | Iterable<File>): Promise<void> {
    if (!this.enabled) return;
    await this.uploadRuntime.uploadFiles(filesInput);
  }

  handleFrame(payload: Uint8Array): void {
    const message = decodeFileMessage(payload);
    const completedDownload = this.downloadRuntime.handleMessage(message);
    if (completedDownload) {
      triggerBrowserDownload(completedDownload);
    }
  }
}

export function encodeFileHeader(message: {
  id: number;
  filename: string;
  size: number;
  mime: string;
}): Uint8Array {
  return FileTransferCodec.encodeHeader(message);
}

export function encodeFileChunk(message: {
  id: number;
  seq: number;
  data: Uint8Array;
}): Uint8Array {
  return FileTransferCodec.encodeChunk(message);
}

export function encodeFileComplete(id: number): Uint8Array {
  return FileTransferCodec.encodeComplete(id);
}

export function decodeFileMessage(payload: Uint8Array): DecodedFileMessage {
  return FileTransferCodec.decode(payload);
}

function triggerBrowserDownload(download: CompletedDownload): void {
  const blob = new Blob(download.chunks, {
    type: download.mime || 'application/octet-stream',
  });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = download.filename;
  anchor.style.display = 'none';
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}
