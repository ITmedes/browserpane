import { describe, expect, it } from 'vitest';
import { JwksWireMapper } from './jwks-wire-mapper';

describe('JwksWireMapper', () => {
  it('maps supported public signing keys and ignores encryption keys', () => {
    expect(JwksWireMapper.toPublicKeySet({
      keys: [
        { kty: 'RSA', kid: 'rsa', use: 'sig', alg: 'RS256', n: 'modulus', e: 'AQAB' },
        { kty: 'EC', kid: 'ec', crv: 'P-256', x: 'x', y: 'y', key_ops: ['verify'] },
        { kty: 'OKP', kid: 'okp', crv: 'Ed25519', x: 'x', x5c: ['certificate'] },
        { kty: 'RSA', kid: 'enc', use: 'enc', alg: 'RSA-OAEP', n: 'modulus', e: 'AQAB' },
      ],
    }).keys).toHaveLength(3);
  });

  it.each([
    [{}, 'bounded non-empty array'],
    [{ keys: [] }, 'bounded non-empty array'],
    [{ keys: [{ kty: 'oct', k: 'secret' }] }, 'private or symmetric'],
    [{ keys: [{ kty: 'AKP' }] }, 'not supported'],
    [{ keys: [{ kty: 'RSA', n: 'n', e: 'e', use: 'enc' }] }, 'supported signing key'],
    [{ keys: [{ kty: 'RSA', n: 'n', e: 'e', use: 'invalid' }] }, 'use must be sig or enc'],
    [{ keys: [{ kty: 'RSA', n: 'n', e: 'e', key_ops: 'verify' }] }, 'string array'],
    [{ keys: [{ kty: 'EC', crv: 'P-256', x: 'x' }] }, 'y coordinate'],
  ])('rejects unsafe or malformed key set %#', (payload, expectedMessage) => {
    expect(() => JwksWireMapper.toPublicKeySet(payload)).toThrow(expectedMessage);
  });
});
