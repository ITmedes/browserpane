import { CH_FILE_UP } from '../protocol.js';
import {
  FileTransferCodec,
} from './codec.js';

const FILE_UPLOAD_CHUNK_SIZE = 64 * 1024;

type SendFrameFn = (channelId: number, payload: Uint8Array) => void;

export class FileUploadRuntime {
  private nextTransferId = 1;
  private readonly sendFrame: SendFrameFn;

  constructor(sendFrame: SendFrameFn) {
    this.sendFrame = sendFrame;
  }

  async uploadFiles(filesInput: FileList | Iterable<File>): Promise<void> {
    const files = normalizeFiles(filesInput);
    for (const file of files) {
      await this.uploadFile(file);
    }
  }

  private async uploadFile(file: File): Promise<void> {
    const id = this.nextTransferId++;
    this.sendFrame(
      CH_FILE_UP,
      FileTransferCodec.encodeHeader({
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
      this.sendFrame(CH_FILE_UP, FileTransferCodec.encodeChunk({ id, seq, data: chunk }));
      offset += chunk.byteLength;
      seq += 1;
    }

    this.sendFrame(CH_FILE_UP, FileTransferCodec.encodeComplete(id));
  }
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
