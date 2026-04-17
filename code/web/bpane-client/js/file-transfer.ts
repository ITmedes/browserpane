import { CH_FILE_DOWN, CH_FILE_UP } from './protocol.js';
import { FileTransferCodec, type DecodedFileMessage } from './file-transfer/codec.js';
import {
  FileDownloadRuntime,
  type CompletedDownload,
} from './file-transfer/download-runtime.js';
import { FileUploadRuntime } from './file-transfer/upload-runtime.js';

type SendFrameFn = (channelId: number, payload: Uint8Array) => void;

interface FileTransferOptions {
  container: HTMLElement;
  enabled: boolean;
  sendFrame: SendFrameFn;
}

export class FileTransferController {
  private container: HTMLElement;
  private enabled: boolean;
  private fileInput: HTMLInputElement | null = null;
  private dragDepth = 0;
  private readonly downloadRuntime = new FileDownloadRuntime();
  private readonly uploadRuntime: FileUploadRuntime;

  private readonly handleInputChange = (): void => {
    const files = this.fileInput?.files;
    if (files) {
      void this.uploadFiles(files);
    }
  };

  private readonly handleDragEnter = (event: DragEvent): void => {
    if (!this.enabled || !hasFilePayload(event)) return;
    event.preventDefault();
    this.dragDepth += 1;
  };

  private readonly handleDragOver = (event: DragEvent): void => {
    if (!this.enabled || !hasFilePayload(event)) return;
    event.preventDefault();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = 'copy';
    }
  };

  private readonly handleDragLeave = (event: DragEvent): void => {
    if (!this.enabled || !hasFilePayload(event)) return;
    event.preventDefault();
    this.dragDepth = Math.max(0, this.dragDepth - 1);
  };

  private readonly handleDrop = (event: DragEvent): void => {
    if (!this.enabled || !hasFilePayload(event)) return;
    event.preventDefault();
    this.dragDepth = 0;
    const files = event.dataTransfer?.files;
    if (files) {
      void this.uploadFiles(files);
    }
  };

  constructor(options: FileTransferOptions) {
    this.container = options.container;
    this.enabled = options.enabled;
    this.uploadRuntime = new FileUploadRuntime(options.sendFrame);
    this.setup();
  }

  destroy(): void {
    this.container.removeEventListener('dragenter', this.handleDragEnter);
    this.container.removeEventListener('dragover', this.handleDragOver);
    this.container.removeEventListener('dragleave', this.handleDragLeave);
    this.container.removeEventListener('drop', this.handleDrop);
    if (this.fileInput) {
      this.fileInput.removeEventListener('change', this.handleInputChange);
      if (this.fileInput.parentNode) {
        this.fileInput.parentNode.removeChild(this.fileInput);
      }
      this.fileInput = null;
    }
    this.downloadRuntime.destroy();
  }

  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
    if (this.fileInput) {
      this.fileInput.disabled = !enabled;
    }
  }

  promptUpload(): void {
    if (!this.enabled || !this.fileInput) return;
    this.fileInput.value = '';
    this.fileInput.click();
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

  private setup(): void {
    this.fileInput = document.createElement('input');
    this.fileInput.type = 'file';
    this.fileInput.multiple = true;
    this.fileInput.style.display = 'none';
    this.fileInput.disabled = !this.enabled;
    this.fileInput.addEventListener('change', this.handleInputChange);
    this.container.appendChild(this.fileInput);

    this.container.addEventListener('dragenter', this.handleDragEnter);
    this.container.addEventListener('dragover', this.handleDragOver);
    this.container.addEventListener('dragleave', this.handleDragLeave);
    this.container.addEventListener('drop', this.handleDrop);
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

function hasFilePayload(event: DragEvent): boolean {
  const types = event.dataTransfer?.types;
  return !!types && Array.from(types).includes('Files');
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
