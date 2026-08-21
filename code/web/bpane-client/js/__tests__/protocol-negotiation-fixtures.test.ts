import { describe, expect, it } from 'vitest';

import {
  CH_CONTROL,
  ProtocolNegotiationCodec,
  ProtocolNegotiationError,
  ProtocolNegotiator,
  encodeFrame,
  parseFrames,
  type ProtocolFailureCode,
} from '../protocol.js';
import { ProtocolNegotiationFixtureParser } from './protocol-negotiation-fixture-parser.js';
import type { SelectionFixture } from './protocol-negotiation-fixture-types.js';
import {
  WireFixtureCatalogParser,
  wireFixtureCatalog,
} from './wire-fixtures.js';

const EXPECTED_CURRENT_VECTORS = 15;
const EXPECTED_NEGOTIATION_VECTORS = 41;
const EXPECTED_SELECTION_VECTORS = 10;

describe('shared protocol negotiation conformance vectors', () => {
  it('enumerates every message vector with exact bytes and fixed outcomes', () => {
    const catalog = wireFixtureCatalog();
    const parser = new ProtocolNegotiationFixtureParser();
    const codec = new ProtocolNegotiationCodec();
    expect(catalog.negotiation_vectors).toHaveLength(EXPECTED_NEGOTIATION_VECTORS);

    for (const [index, raw] of catalog.negotiation_vectors.entries()) {
      const vector = parser.parseNegotiation(raw, index);
      const wire = decodeHex(vector.name, vector.wireHex);
      const [frames, remaining] = parseFrames(wire);
      expect(remaining, vector.name).toHaveLength(0);
      expect(frames, vector.name).toHaveLength(1);
      expect(frames[0].channelId, vector.name).toBe(CH_CONTROL);

      if (vector.expected.outcome === 'valid') {
        const decoded = codec.decode(frames[0].payload);
        expect(decoded, vector.name).toEqual(vector.expected.message);
        expect(
          encodeFrame(CH_CONTROL, codec.encode(decoded)),
          `${vector.name} encoder bytes`,
        ).toEqual(wire);
      } else {
        expectFailure(
          () => codec.decode(frames[0].payload),
          vector.expected.error,
          vector.name,
        );
      }
    }
  });

  it('enumerates every pure selection and validation outcome', () => {
    const catalog = wireFixtureCatalog();
    const parser = new ProtocolNegotiationFixtureParser();
    const negotiator = new ProtocolNegotiator();
    expect(catalog.selection_vectors).toHaveLength(EXPECTED_SELECTION_VECTORS);

    for (const [index, raw] of catalog.selection_vectors.entries()) {
      const vector = parser.parseSelection(raw, index);
      const actual = runSelectionVector(negotiator, vector);
      expect(actual, vector.name).toEqual(vector.expected);
    }
  });

  it('keeps every name unique across the complete schema-v2 corpus', () => {
    const catalog = wireFixtureCatalog();
    const parser = new ProtocolNegotiationFixtureParser();
    expect(catalog.vectors).toHaveLength(EXPECTED_CURRENT_VECTORS);
    const names = [
      ...catalog.vectors.map((vector) => vector.name),
      ...catalog.negotiation_vectors.map(
        (raw, index) => parser.parseNegotiation(raw, index).name,
      ),
      ...catalog.selection_vectors.map(
        (raw, index) => parser.parseSelection(raw, index).name,
      ),
    ];
    expect(names).toHaveLength(66);
    expect(new Set(names).size).toBe(names.length);
  });

  it('fails closed on schema mismatches and unknown outcomes', () => {
    const parser = new WireFixtureCatalogParser();
    expect(() => parser.parse({
      schema_version: 3,
      catalog: 'browserpane-v1-conformance',
      vectors: [],
      negotiation_vectors: [],
      selection_vectors: [],
    })).toThrow('invalid wire fixture catalog header');

    const vectorParser = new ProtocolNegotiationFixtureParser();
    expect(() => vectorParser.parseNegotiation({
      name: 'unknown-outcome',
      direction: 'client_to_server',
      wire_hex: '00',
      expected: { outcome: 'unknown' },
    }, 0)).toThrow('invalid negotiation outcome');
    expect(() => vectorParser.parseSelection({
      name: 'unknown-outcome',
      operation: 'select',
      hello: {
        versions: [1],
        required_capabilities: [],
        optional_capabilities: [],
      },
      support: { versions: [1], capabilities: [] },
      expected: { outcome: 'unknown' },
    }, 0)).toThrow('invalid selection outcome');
  });
});

function runSelectionVector(
  negotiator: ProtocolNegotiator,
  vector: SelectionFixture,
): SelectionFixture['expected'] {
  try {
    if (vector.operation === 'select') {
      return { outcome: 'selected', selection: negotiator.select(vector.hello, vector.support) };
    }
    if (vector.selection === undefined) {
      throw new Error(`missing selection for ${vector.name}`);
    }
    negotiator.validateSelection(
      vector.hello,
      vector.support,
      vector.selection,
    );
    return { outcome: 'accepted' };
  } catch (error) {
    if (error instanceof ProtocolNegotiationError) {
      return { outcome: 'rejected', error: error.code };
    }
    throw error;
  }
}

function expectFailure(
  action: () => unknown,
  code: ProtocolFailureCode,
  name: string,
): void {
  try {
    action();
    throw new Error(`expected negotiation failure for ${name}`);
  } catch (error) {
    if (!(error instanceof ProtocolNegotiationError)) {
      throw error;
    }
    expect(error.code, name).toBe(code);
  }
}

function decodeHex(name: string, hex: string): Uint8Array {
  if (hex.length % 2 !== 0 || !/^[0-9a-f]+$/.test(hex)) {
    throw new Error(`fixture ${name} has invalid lowercase hex`);
  }
  const output = new Uint8Array(hex.length / 2);
  for (let index = 0; index < output.length; index += 1) {
    output[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16);
  }
  return output;
}
