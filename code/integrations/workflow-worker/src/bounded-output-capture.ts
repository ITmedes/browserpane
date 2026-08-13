const UTF8_CONTINUATION_MASK = 0b1100_0000;
const UTF8_CONTINUATION_PREFIX = 0b1000_0000;

export type BoundedOutputResult = {
  text: string;
  capturedBytes: number;
  omittedBytes: number;
  truncated: boolean;
};

export class BoundedOutputCapture {
  private readonly bytes: Buffer;
  private length = 0;
  private writeOffset = 0;
  private totalBytes = 0;

  constructor(maxBytes: number) {
    if (!Number.isSafeInteger(maxBytes) || maxBytes <= 0) {
      throw new Error("worker output limit must be a positive integer");
    }
    this.bytes = Buffer.allocUnsafe(maxBytes);
  }

  append(chunk: Uint8Array): void {
    const source = Buffer.from(chunk.buffer, chunk.byteOffset, chunk.byteLength);
    this.totalBytes += source.length;
    if (source.length >= this.bytes.length) {
      source.copy(this.bytes, 0, source.length - this.bytes.length);
      this.length = this.bytes.length;
      this.writeOffset = 0;
      return;
    }

    const firstLength = Math.min(source.length, this.bytes.length - this.writeOffset);
    source.copy(this.bytes, this.writeOffset, 0, firstLength);
    if (firstLength < source.length) {
      source.copy(this.bytes, 0, firstLength);
    }
    this.writeOffset = (this.writeOffset + source.length) % this.bytes.length;
    this.length = Math.min(this.bytes.length, this.length + source.length);
  }

  result(): BoundedOutputResult {
    const captured = this.orderedBytes();
    const omittedBytes = Math.max(0, this.totalBytes - this.length);
    const decoded = this.decodeUtf8Tail(captured);
    const marker = omittedBytes > 0
      ? `[BrowserPane: omitted ${omittedBytes} earlier output bytes]\n`
      : "";
    return {
      text: `${marker}${decoded}`,
      capturedBytes: this.length,
      omittedBytes,
      truncated: omittedBytes > 0,
    };
  }

  private orderedBytes(): Buffer {
    if (this.length < this.bytes.length) {
      return Buffer.from(this.bytes.subarray(0, this.length));
    }
    return Buffer.concat([
      this.bytes.subarray(this.writeOffset),
      this.bytes.subarray(0, this.writeOffset),
    ]);
  }

  private decodeUtf8Tail(value: Buffer): string {
    let start = 0;
    while (
      start < value.length
      && (value[start]! & UTF8_CONTINUATION_MASK) === UTF8_CONTINUATION_PREFIX
    ) {
      start += 1;
    }
    return value.subarray(start).toString("utf8");
  }
}

export class BoundedStreamReader {
  private readonly maxBytes: number;

  constructor(maxBytes: number) {
    this.maxBytes = maxBytes;
  }

  async read(stream: NodeJS.ReadableStream | null): Promise<BoundedOutputResult> {
    const capture = new BoundedOutputCapture(this.maxBytes);
    if (!stream) {
      return capture.result();
    }
    for await (const chunk of stream) {
      capture.append(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    }
    return capture.result();
  }
}
