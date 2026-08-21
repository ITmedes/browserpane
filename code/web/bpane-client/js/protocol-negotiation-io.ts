import {
  ProtocolNegotiationError,
  type ProtocolFailureCode,
} from './protocol-negotiation-types.js';

/** Bounded little-endian reader used only after a message-size check. */
export class NegotiationReader {
  private position = 0;

  public constructor(
    private readonly payload: Uint8Array,
    private readonly failure: ProtocolFailureCode,
  ) {}

  public readUint8(): number {
    this.require(1);
    return this.payload[this.position++];
  }

  public readUint16(): number {
    this.require(2);
    const value = this.payload[this.position] | (this.payload[this.position + 1] << 8);
    this.position += 2;
    return value;
  }

  public readList(maximum: number): number[] {
    const count = this.readUint8();
    if (count > maximum) {
      throw new ProtocolNegotiationError(this.failure);
    }
    const values: number[] = [];
    for (let index = 0; index < count; index += 1) {
      values.push(this.readUint16());
    }
    return values;
  }

  public finish(): void {
    if (this.position !== this.payload.length) {
      throw new ProtocolNegotiationError(this.failure);
    }
  }

  private require(length: number): void {
    if (this.position + length > this.payload.length) {
      throw new ProtocolNegotiationError(this.failure);
    }
  }
}

/** Exact-size little-endian writer for validated negotiation messages. */
export class NegotiationWriter {
  private position = 0;

  public constructor(private readonly output: Uint8Array) {}

  public writeUint8(value: number): void {
    this.output[this.position++] = value;
  }

  public writeUint16(value: number): void {
    this.output[this.position++] = value & 0xFF;
    this.output[this.position++] = value >>> 8;
  }

  public writeList(values: readonly number[]): void {
    this.writeUint8(values.length);
    for (const value of values) {
      this.writeUint16(value);
    }
  }
}
