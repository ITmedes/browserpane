/**
 * FNV-1a hash for clipboard echo prevention.
 * Must match the Rust implementation in bpane-host/src/clipboard.rs.
 */
export function fnvHash(text: string): bigint {
  const encoder = new TextEncoder();
  const data = encoder.encode(text);
  let hash = 0xcbf29ce484222325n;
  for (let i = 0; i < data.length; i++) {
    hash ^= BigInt(data[i]);
    hash = (hash * 0x100000001b3n) & 0xFFFFFFFFFFFFFFFFn;
  }
  return hash;
}
