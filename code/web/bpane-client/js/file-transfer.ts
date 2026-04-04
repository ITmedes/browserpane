import { CH_FILE_DOWN, CH_FILE_UP } from './protocol.js';

const FILE_HEADER = 0x01;
const FILE_CHUNK = 0x02;
const FILE_COMPLETE = 0x03;
const FILE_NAME_BYTES = 256;
const FILE_MIME_BYTES = 64;
const FILE_UPLOAD_CHUNK_SIZE = 64 * 1024;

type SendFrameFn = (channelId: number, payload: Uint8Array) => void;

interface FileTransferOptions {
  container: HTMLElement;
  enabled: boolean;
  sendFrame: SendFrameFn;
}

interface DownloadState {
  filename: string;
  mime: string;
  expectedSize: number;
  receivedSize: number;
  expectedSeq: number;
  chunks: ArrayBuffer[];
}

export type DecodedFileMessage =
  | { type: 'header'; id: number; filename: string; size: number; mime: string }
  | { type: 'chunk'; id: number; seq: number; data: Uint8Array }
  | { type: 'complete'; id: number };

export class FileTransferController {
  private container: HTMLElement;
  private enabled: boolean;
  private sendFrame: SendFrameFn;
  private fileInput: HTMLInputElement | null = null;
  private dragDepth = 0;
  private nextTransferId = 1;
  private readonly activeDownloads = new Map<number, DownloadState>();

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
    this.sendFrame = options.sendFrame;
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
    this.activeDownloads.clear();
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
    const files = normalizeFiles(filesInput);
    for (const file of files) {
      await this.uploadFile(file);
    }
  }

  handleFrame(payload: Uint8Array): void {
    const message = decodeFileMessage(payload);
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
        break;
      case 'chunk':
        this.handleDownloadChunk(message.id, message.seq, message.data);
        break;
      case 'complete':
        this.completeDownload(message.id);
        break;
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

  private async uploadFile(file: File): Promise<void> {
    const id = this.nextTransferId++;
    this.sendFrame(
      CH_FILE_UP,
      encodeFileHeader({
        id,
        filename: file.name || `upload-${id}`,
        size: file.size,
        mime: file.type || 'application/octet-stream',
      }),
    );

    let offset = 0;
    let seq = 0;
    while (offset < file.size) {
      const chunk = new Uint8Array(
        await file.slice(offset, offset + FILE_UPLOAD_CHUNK_SIZE).arrayBuffer(),
      );
      this.sendFrame(CH_FILE_UP, encodeFileChunk({ id, seq, data: chunk }));
      offset += chunk.byteLength;
      seq += 1;
    }

    this.sendFrame(CH_FILE_UP, encodeFileComplete(id));
  }

  private handleDownloadChunk(id: number, seq: number, data: Uint8Array): void {
    const download = this.activeDownloads.get(id);
    if (!download) {
      console.warn('[bpane] dropped file chunk without header', { id, seq });
      return;
    }
    if (seq !== download.expectedSeq) {
      console.warn('[bpane] file chunk sequence mismatch', {
        id,
        expectedSeq: download.expectedSeq,
        seq,
      });
      this.activeDownloads.delete(id);
      return;
    }

    download.chunks.push(new Uint8Array(data).buffer);
    download.receivedSize += data.byteLength;
    download.expectedSeq += 1;
  }

  private completeDownload(id: number): void {
    const download = this.activeDownloads.get(id);
    if (!download) {
      console.warn('[bpane] dropped file completion without header', { id });
      return;
    }
    this.activeDownloads.delete(id);

    if (download.expectedSize > 0 && download.receivedSize !== download.expectedSize) {
      console.warn('[bpane] file download size mismatch', {
        id,
        expectedSize: download.expectedSize,
        receivedSize: download.receivedSize,
      });
    }

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
}

export function encodeFileHeader(message: {
  id: number;
  filename: string;
  size: number;
  mime: string;
}): Uint8Array {
  const payload = new Uint8Array(1 + 4 + FILE_NAME_BYTES + 8 + FILE_MIME_BYTES);
  const view = new DataView(payload.buffer);
  payload[0] = FILE_HEADER;
  view.setUint32(1, message.id >>> 0, true);
  payload.set(encodeFixedString(message.filename, FILE_NAME_BYTES), 5);
  view.setBigUint64(5 + FILE_NAME_BYTES, BigInt(message.size), true);
  payload.set(encodeFixedString(message.mime, FILE_MIME_BYTES), 13 + FILE_NAME_BYTES);
  return payload;
}

export function encodeFileChunk(message: {
  id: number;
  seq: number;
  data: Uint8Array;
}): Uint8Array {
  const payload = new Uint8Array(1 + 4 + 4 + 4 + message.data.byteLength);
  const view = new DataView(payload.buffer);
  payload[0] = FILE_CHUNK;
  view.setUint32(1, message.id >>> 0, true);
  view.setUint32(5, message.seq >>> 0, true);
  view.setUint32(9, message.data.byteLength >>> 0, true);
  payload.set(message.data, 13);
  return payload;
}

export function encodeFileComplete(id: number): Uint8Array {
  const payload = new Uint8Array(1 + 4);
  const view = new DataView(payload.buffer);
  payload[0] = FILE_COMPLETE;
  view.setUint32(1, id >>> 0, true);
  return payload;
}

export function decodeFileMessage(payload: Uint8Array): DecodedFileMessage {
  if (payload.byteLength < 1) {
    throw new Error('file payload too short');
  }

  const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
  const tag = payload[0];
  if (tag === FILE_HEADER) {
    if (payload.byteLength < 1 + 4 + FILE_NAME_BYTES + 8 + FILE_MIME_BYTES) {
      throw new Error('file header too short');
    }
    return {
      type: 'header',
      id: view.getUint32(1, true),
      filename: decodeFixedString(payload.subarray(5, 5 + FILE_NAME_BYTES)),
      size: Number(view.getBigUint64(5 + FILE_NAME_BYTES, true)),
      mime: decodeFixedString(payload.subarray(13 + FILE_NAME_BYTES, 13 + FILE_NAME_BYTES + FILE_MIME_BYTES)),
    };
  }
  if (tag === FILE_CHUNK) {
    if (payload.byteLength < 13) {
      throw new Error('file chunk too short');
    }
    const length = view.getUint32(9, true);
    if (payload.byteLength < 13 + length) {
      throw new Error('file chunk truncated');
    }
    return {
      type: 'chunk',
      id: view.getUint32(1, true),
      seq: view.getUint32(5, true),
      data: payload.subarray(13, 13 + length),
    };
  }
  if (tag === FILE_COMPLETE) {
    if (payload.byteLength < 5) {
      throw new Error('file completion too short');
    }
    return {
      type: 'complete',
      id: view.getUint32(1, true),
    };
  }

  throw new Error(`unknown file tag: ${tag}`);
}

function normalizeFiles(filesInput: FileList | Iterable<File>): File[] {
  if (typeof (filesInput as FileList).item === 'function') {
    const files = filesInput as FileList;
    const normalized: File[] = [];
    for (let index = 0; index < files.length; index += 1) {
      const file = files.item(index);
      if (file) normalized.push(file);
    }
    return normalized;
  }
  if (Symbol.iterator in Object(filesInput)) {
    return Array.from(filesInput as Iterable<File>);
  }
  if (typeof (filesInput as ArrayLike<File>).length === 'number') {
    return Array.from(filesInput as ArrayLike<File>).filter((file): file is File => !!file);
  }
  return Array.from(filesInput);
}

function hasFilePayload(event: DragEvent): boolean {
  const types = event.dataTransfer?.types;
  return !!types && Array.from(types).includes('Files');
}

function encodeFixedString(input: string, maxBytes: number): Uint8Array {
  const output = new Uint8Array(maxBytes);
  const encoder = new TextEncoder();
  let offset = 0;
  for (const char of input) {
    const encoded = encoder.encode(char);
    if (offset + encoded.byteLength > maxBytes) {
      break;
    }
    output.set(encoded, offset);
    offset += encoded.byteLength;
  }
  return output;
}

function decodeFixedString(bytes: Uint8Array): string {
  let end = bytes.indexOf(0);
  if (end < 0) end = bytes.byteLength;
  return new TextDecoder().decode(bytes.subarray(0, end)).trim();
}
