import { readFileSync } from 'node:fs';

export type WireFixtureExpectation =
  | { outcome: 'valid'; message: string }
  | { outcome: 'invalid'; error: string };

export type WireFixture = {
  name: string;
  direction: 'client_to_server' | 'server_to_client' | 'bidirectional';
  transport: 'reliable_frame' | 'datagram_payload';
  channel: string;
  capabilities: string[];
  wire_hex: string;
  expected: WireFixtureExpectation;
};

export type WireFixtureCatalog = {
  schema_version: number;
  catalog: string;
  vectors: WireFixture[];
  negotiation_vectors: unknown[];
  selection_vectors: unknown[];
};

const FIXTURE_PATH = `${process.cwd()}/../../shared/bpane-protocol/tests/fixtures/wire-fixtures.json`;
let cache: WireFixtureCatalog | null = null;

export function wireFixtureCatalog(): WireFixtureCatalog {
  if (!cache) {
    cache = new WireFixtureCatalogParser().parse(
      JSON.parse(readFileSync(FIXTURE_PATH, 'utf8')) as unknown,
    );
  }
  return cache;
}

export function wireFixture(name: string): Uint8Array {
  const fixture = wireFixtureCatalog().vectors.find((candidate) => candidate.name === name);
  if (!fixture) {
    throw new Error(`missing wire fixture: ${name}`);
  }
  return decodeHex(fixture.name, fixture.wire_hex);
}

export class WireFixtureCatalogParser {
  public parse(input: unknown): WireFixtureCatalog {
    if (!isRecord(input)
      || input.schema_version !== 2
      || input.catalog !== 'browserpane-v1-conformance'
      || !Array.isArray(input.vectors)
      || !Array.isArray(input.negotiation_vectors)
      || !Array.isArray(input.selection_vectors)) {
      throw new Error('invalid wire fixture catalog header');
    }

    return {
      schema_version: input.schema_version,
      catalog: input.catalog,
      vectors: input.vectors.map(parseVector),
      negotiation_vectors: input.negotiation_vectors,
      selection_vectors: input.selection_vectors,
    };
  }
}

function parseVector(input: unknown, index: number): WireFixture {
  if (!isRecord(input)) {
    throw new Error(`invalid wire fixture at index ${index}`);
  }
  const direction = input.direction;
  const transport = input.transport;
  if (!isDirection(direction) || !isTransport(transport)) {
    throw new Error(`invalid wire fixture metadata at index ${index}`);
  }
  if (typeof input.name !== 'string'
    || typeof input.channel !== 'string'
    || typeof input.wire_hex !== 'string'
    || !Array.isArray(input.capabilities)
    || !input.capabilities.every((value) => typeof value === 'string')) {
    throw new Error(`invalid wire fixture fields at index ${index}`);
  }

  return {
    name: input.name,
    direction,
    transport,
    channel: input.channel,
    capabilities: input.capabilities,
    wire_hex: input.wire_hex,
    expected: parseExpectation(input.expected, index),
  };
}

function parseExpectation(input: unknown, index: number): WireFixtureExpectation {
  if (!isRecord(input)) {
    throw new Error(`invalid wire fixture expectation at index ${index}`);
  }
  if (input.outcome === 'valid' && typeof input.message === 'string') {
    return { outcome: input.outcome, message: input.message };
  }
  if (input.outcome === 'invalid' && typeof input.error === 'string') {
    return { outcome: input.outcome, error: input.error };
  }
  throw new Error(`invalid wire fixture outcome at index ${index}`);
}

function decodeHex(name: string, hex: string): Uint8Array {
  if (hex.length % 2 !== 0 || !/^[0-9a-f]+$/.test(hex)) {
    throw new Error(`fixture ${name} has invalid lowercase hex`);
  }

  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i += 1) {
    out[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

function isRecord(input: unknown): input is Record<string, unknown> {
  return typeof input === 'object' && input !== null && !Array.isArray(input);
}

function isDirection(input: unknown): input is WireFixture['direction'] {
  return input === 'client_to_server'
    || input === 'server_to_client'
    || input === 'bidirectional';
}

function isTransport(input: unknown): input is WireFixture['transport'] {
  return input === 'reliable_frame' || input === 'datagram_payload';
}
