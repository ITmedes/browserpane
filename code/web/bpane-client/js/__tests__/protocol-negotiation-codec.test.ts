import { describe, expect, it } from 'vitest';

import {
  PROTOCOL_FAILURE_ID,
  ProtocolNegotiationCodec,
  ProtocolNegotiationError,
  ProtocolNegotiationRegistry,
  encodeFrame,
  CH_CONTROL,
  type ClientHello,
  type ProtocolFailureCode,
  type ServerSelection,
} from '../protocol.js';

const codec = new ProtocolNegotiationCodec();

describe('ProtocolNegotiationCodec', () => {
  it('encodes and decodes exact hello, selection, and rejection bytes', () => {
    const hello: ClientHello = {
      versions: [1],
      requiredCapabilities: [0x000B],
      optionalCapabilities: [0x0004, 0x0005],
    };
    expect(codec.encodeClientHello(hello)).toEqual(new Uint8Array([
      0x0A, 0x01, 0x01, 0x00, 0x01, 0x0B, 0x00, 0x02, 0x04, 0x00, 0x05, 0x00,
    ]));
    expect(codec.decodeClientHello(codec.encodeClientHello(hello))).toEqual(hello);

    const selection: ServerSelection = {
      selectedVersion: 1,
      capabilities: [0x0004, 0x0005, 0x000B],
    };
    expect(codec.encodeServerSelection(selection)).toEqual(new Uint8Array([
      0x0B, 0x01, 0x00, 0x03, 0x04, 0x00, 0x05, 0x00, 0x0B, 0x00,
    ]));
    expect(codec.decodeServerSelection(codec.encodeServerSelection(selection))).toEqual(
      selection,
    );

    const rejection = codec.encodeProtocolReject('required_protocol_capability_missing');
    expect(rejection).toEqual(new Uint8Array([0x0C, 0x02, 0x00]));
    expect(codec.decodeProtocolReject(rejection)).toBe(
      'required_protocol_capability_missing',
    );
  });

  it('accepts the exact maximum client hello without over-allocation', () => {
    const maximum: ClientHello = {
      versions: Array.from({ length: 8 }, (_, index) => index + 1),
      requiredCapabilities: [],
      optionalCapabilities: Array.from({ length: 64 }, (_, index) => index + 1),
    };
    const encoded = codec.encodeClientHello(maximum);
    expect(encoded).toHaveLength(148);
    expect(codec.decodeClientHello(encoded)).toEqual(maximum);
  });

  it('rejects non-canonical, overlapping, malformed, and oversized hellos', () => {
    const invalid: ClientHello[] = [
      { versions: [], requiredCapabilities: [], optionalCapabilities: [] },
      { versions: [0], requiredCapabilities: [], optionalCapabilities: [] },
      { versions: [2, 1], requiredCapabilities: [], optionalCapabilities: [] },
      { versions: [1, 1], requiredCapabilities: [], optionalCapabilities: [] },
      { versions: [1], requiredCapabilities: [2, 1], optionalCapabilities: [] },
      { versions: [1], requiredCapabilities: [1], optionalCapabilities: [1] },
      {
        versions: [1],
        requiredCapabilities: Array.from({ length: 64 }, (_, index) => index + 1),
        optionalCapabilities: [65],
      },
    ];
    for (const hello of invalid) {
      expectFailure(() => codec.encodeClientHello(hello), 'malformed_protocol_hello');
    }
  });

  it('maps every truncated and trailing hello mutation to one fixed outcome', () => {
    const valid = codec.encodeClientHello({
      versions: [1],
      requiredCapabilities: [],
      optionalCapabilities: [],
    });
    for (let end = 1; end < valid.length; end += 1) {
      expectFailure(
        () => codec.decodeClientHello(valid.slice(0, end)),
        'malformed_protocol_hello',
      );
    }
    const trailing = new Uint8Array(valid.length + 1);
    trailing.set(valid);
    expectFailure(
      () => codec.decodeClientHello(trailing),
      'malformed_protocol_hello',
    );
  });

  it('rejects invalid selection structure and unknown rejection values', () => {
    const invalid: ServerSelection[] = [
      { selectedVersion: 0, capabilities: [] },
      { selectedVersion: 1, capabilities: [2, 1] },
      { selectedVersion: 1, capabilities: [1, 1] },
      { selectedVersion: 1, capabilities: [0x0005] },
      { selectedVersion: 1, capabilities: [0x0006, 0x0008] },
      { selectedVersion: 1, capabilities: [0xFFFF] },
    ];
    for (const selection of invalid) {
      expectFailure(
        () => codec.encodeServerSelection(selection),
        'protocol_selection_mismatch',
      );
    }
    expectFailure(
      () => codec.decodeProtocolReject(new Uint8Array([0x0C, 0x0A, 0x00])),
      'unexpected_protocol_frame',
    );
    expectFailure(
      () => codec.decode(new Uint8Array([0xFF])),
      'unexpected_protocol_frame',
    );
  });

  it('roundtrips all failures and deterministic generated hellos', () => {
    for (const id of Object.values(PROTOCOL_FAILURE_ID)) {
      const code = ProtocolNegotiationRegistry.failureCode(id);
      if (code === undefined) {
        throw new Error(`missing protocol failure ${id}`);
      }
      expect(codec.decodeProtocolReject(codec.encodeProtocolReject(code))).toBe(code);
    }

    for (let seed = 0; seed < 128; seed += 1) {
      const requiredCapabilities = [1, 2, 3, 4].filter((id) => (seed & (1 << id)) !== 0);
      const optionalCapabilities = [1, 2, 3, 4]
        .filter((id) => !requiredCapabilities.includes(id))
        .filter((id) => (seed & (1 << (id + 4))) !== 0);
      const hello = { versions: [1], requiredCapabilities, optionalCapabilities };
      expect(codec.decodeClientHello(codec.encodeClientHello(hello))).toEqual(hello);
    }
  });

  it('crosses the production frame boundary byte-for-byte', () => {
    const payload = codec.encodeClientHello({
      versions: [1],
      requiredCapabilities: [0x000B],
      optionalCapabilities: [0x0004, 0x0005],
    });
    expect(encodeFrame(CH_CONTROL, payload)).toEqual(new Uint8Array([
      0x0A, 0x0C, 0x00, 0x00, 0x00,
      0x0A, 0x01, 0x01, 0x00, 0x01, 0x0B, 0x00, 0x02, 0x04, 0x00, 0x05, 0x00,
    ]));
  });
});

function expectFailure(action: () => unknown, code: ProtocolFailureCode): void {
  try {
    action();
    throw new Error('expected protocol negotiation failure');
  } catch (error) {
    if (!(error instanceof ProtocolNegotiationError)) {
      throw error;
    }
    expect(error.code).toBe(code);
    expect(error.message).toBe(code);
  }
}
