import { describe, it, expect } from 'vitest';
import { fnvHash } from '../hash.js';

describe('fnvHash', () => {
  it('returns the FNV offset basis for empty string', () => {
    // FNV-1a offset basis
    expect(fnvHash('')).toBe(0xcbf29ce484222325n);
  });

  it('produces consistent hashes', () => {
    const h1 = fnvHash('hello');
    const h2 = fnvHash('hello');
    expect(h1).toBe(h2);
  });

  it('produces different hashes for different inputs', () => {
    const h1 = fnvHash('hello');
    const h2 = fnvHash('world');
    expect(h1).not.toBe(h2);
  });

  it('handles ASCII text', () => {
    const hash = fnvHash('test');
    expect(typeof hash).toBe('bigint');
    expect(hash).not.toBe(0n);
    expect(hash).not.toBe(0xcbf29ce484222325n); // not the offset basis
  });

  it('handles Unicode text', () => {
    const hash = fnvHash('héllo wörld');
    expect(typeof hash).toBe('bigint');
    // Should be different from plain ASCII version
    expect(hash).not.toBe(fnvHash('hello world'));
  });

  it('handles emoji', () => {
    const hash = fnvHash('hello 🌍');
    expect(typeof hash).toBe('bigint');
    expect(hash).not.toBe(fnvHash('hello'));
  });

  it('hashes single character correctly', () => {
    // FNV-1a for 'a' (0x61):
    // hash = 0xcbf29ce484222325 ^ 0x61
    // hash = hash * 0x100000001b3
    // Verify it is a valid u64 (fits in 64 bits)
    const hash = fnvHash('a');
    expect(hash).toBeGreaterThan(0n);
    expect(hash).toBeLessThanOrEqual(0xFFFFFFFFFFFFFFFFn);
  });

  it('stays within u64 range', () => {
    const inputs = ['', 'a', 'hello world', 'a long string that is longer than most', '🌍🌎🌏'];
    for (const input of inputs) {
      const hash = fnvHash(input);
      expect(hash).toBeGreaterThanOrEqual(0n);
      expect(hash).toBeLessThanOrEqual(0xFFFFFFFFFFFFFFFFn);
    }
  });

  it('is sensitive to byte order', () => {
    expect(fnvHash('ab')).not.toBe(fnvHash('ba'));
  });

  it('is sensitive to trailing/leading whitespace', () => {
    expect(fnvHash('hello')).not.toBe(fnvHash(' hello'));
    expect(fnvHash('hello')).not.toBe(fnvHash('hello '));
  });
});
