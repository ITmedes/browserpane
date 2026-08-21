import {
  ProtocolNegotiationError,
  ProtocolNegotiationRegistry,
  type ClientHello,
  type NegotiationMessage,
  type ProtocolFailureCode,
  type ServerSelection,
} from './protocol-negotiation-types.js';
import { NegotiationReader, NegotiationWriter } from './protocol-negotiation-io.js';
import {
  MAX_PROTOCOL_CAPABILITIES,
  MAX_PROTOCOL_VERSIONS,
  ProtocolNegotiationValidator,
} from './protocol-negotiation-validator.js';

export const CTRL_CLIENT_HELLO = 0x0A;
export const CTRL_SERVER_SELECTION = 0x0B;
export const CTRL_PROTOCOL_REJECT = 0x0C;

const MAX_CLIENT_HELLO_BYTES = 148;
const MAX_SERVER_SELECTION_BYTES = 132;

/** Strict, bounded codec for the three protocol negotiation payloads. */
export class ProtocolNegotiationCodec {
  public encode(message: NegotiationMessage): Uint8Array {
    switch (message.type) {
      case 'client_hello':
        return this.encodeClientHello(message.hello);
      case 'server_selection':
        return this.encodeServerSelection(message.selection);
      case 'protocol_reject':
        return this.encodeProtocolReject(message.failure);
    }
  }

  public decode(payload: Uint8Array): NegotiationMessage {
    const tag = payload[0];
    switch (tag) {
      case CTRL_CLIENT_HELLO:
        return { type: 'client_hello', hello: this.decodeClientHello(payload) };
      case CTRL_SERVER_SELECTION:
        return {
          type: 'server_selection',
          selection: this.decodeServerSelection(payload),
        };
      case CTRL_PROTOCOL_REJECT:
        return { type: 'protocol_reject', failure: this.decodeProtocolReject(payload) };
      default:
        throw new ProtocolNegotiationError('unexpected_protocol_frame');
    }
  }

  public encodeClientHello(hello: ClientHello): Uint8Array {
    ProtocolNegotiationValidator.assertClientHello(hello);
    const output = new Uint8Array(
      4 + 2 * (
        hello.versions.length
        + hello.requiredCapabilities.length
        + hello.optionalCapabilities.length
      ),
    );
    const writer = new NegotiationWriter(output);
    writer.writeUint8(CTRL_CLIENT_HELLO);
    writer.writeList(hello.versions);
    writer.writeList(hello.requiredCapabilities);
    writer.writeList(hello.optionalCapabilities);
    return output;
  }

  public decodeClientHello(payload: Uint8Array): ClientHello {
    const failure = 'malformed_protocol_hello';
    this.assertTagAndMaximum(payload, CTRL_CLIENT_HELLO, MAX_CLIENT_HELLO_BYTES, failure);
    const reader = new NegotiationReader(payload, failure);
    reader.readUint8();
    const versions = reader.readList(MAX_PROTOCOL_VERSIONS);
    const requiredCapabilities = reader.readList(MAX_PROTOCOL_CAPABILITIES);
    const optionalCapabilities = reader.readList(
      MAX_PROTOCOL_CAPABILITIES - requiredCapabilities.length,
    );
    reader.finish();
    const hello = { versions, requiredCapabilities, optionalCapabilities };
    ProtocolNegotiationValidator.assertClientHello(hello);
    return hello;
  }

  public encodeServerSelection(selection: ServerSelection): Uint8Array {
    ProtocolNegotiationValidator.assertServerSelection(selection);
    const output = new Uint8Array(4 + 2 * selection.capabilities.length);
    const writer = new NegotiationWriter(output);
    writer.writeUint8(CTRL_SERVER_SELECTION);
    writer.writeUint16(selection.selectedVersion);
    writer.writeList(selection.capabilities);
    return output;
  }

  public decodeServerSelection(payload: Uint8Array): ServerSelection {
    const failure = 'protocol_selection_mismatch';
    this.assertTagAndMaximum(
      payload,
      CTRL_SERVER_SELECTION,
      MAX_SERVER_SELECTION_BYTES,
      failure,
    );
    const reader = new NegotiationReader(payload, failure);
    reader.readUint8();
    const selectedVersion = reader.readUint16();
    const capabilities = reader.readList(MAX_PROTOCOL_CAPABILITIES);
    reader.finish();
    const selection = { selectedVersion, capabilities };
    ProtocolNegotiationValidator.assertServerSelection(selection);
    return selection;
  }

  public encodeProtocolReject(failure: ProtocolFailureCode): Uint8Array {
    const output = new Uint8Array(3);
    const writer = new NegotiationWriter(output);
    writer.writeUint8(CTRL_PROTOCOL_REJECT);
    writer.writeUint16(ProtocolNegotiationRegistry.failureId(failure));
    return output;
  }

  public decodeProtocolReject(payload: Uint8Array): ProtocolFailureCode {
    if (payload.length !== 3 || payload[0] !== CTRL_PROTOCOL_REJECT) {
      throw new ProtocolNegotiationError('unexpected_protocol_frame');
    }
    const id = payload[1] | (payload[2] << 8);
    const failure = ProtocolNegotiationRegistry.failureCode(id);
    if (failure === undefined) {
      throw new ProtocolNegotiationError('unexpected_protocol_frame');
    }
    return failure;
  }

  private assertTagAndMaximum(
    payload: Uint8Array,
    tag: number,
    maximum: number,
    failure: ProtocolFailureCode,
  ): void {
    if (payload[0] !== tag || payload.length > maximum) {
      throw new ProtocolNegotiationError(failure);
    }
  }
}
