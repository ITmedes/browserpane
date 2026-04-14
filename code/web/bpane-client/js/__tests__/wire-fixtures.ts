import { readFileSync } from 'node:fs';

let cache: Record<string, string> | null = null;
const FIXTURE_PATH = `${process.cwd()}/../../shared/bpane-protocol/tests/fixtures/wire-fixtures.json`;

function fixtures(): Record<string, string> {
  if (!cache) {
    cache = JSON.parse(readFileSync(FIXTURE_PATH, 'utf8')) as Record<string, string>;
  }
  return cache;
}

export function wireFixture(name: string): Uint8Array {
  const hex = fixtures()[name];
  if (!hex) {
    throw new Error(`missing wire fixture: ${name}`);
  }
  if (hex.length % 2 !== 0) {
    throw new Error(`fixture ${name} has odd hex length`);
  }

  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i += 1) {
    out[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}
