import type { JSONWebKeySet, JWK } from 'jose';

const SUPPORTED_KEY_TYPES = new Set(['RSA', 'EC', 'OKP']);
const PRIVATE_KEY_FIELDS = ['d', 'p', 'q', 'dp', 'dq', 'qi', 'k'] as const;

export class JwksWireMapper {
  static toPublicKeySet(payload: unknown): JSONWebKeySet {
    const object = expectRecord(payload, 'OIDC JWKS');
    if (!Array.isArray(object.keys) || object.keys.length === 0 || object.keys.length > 100) {
      throw new Error('OIDC JWKS keys must be a bounded non-empty array');
    }
    return { keys: object.keys.map((value) => this.#toPublicKey(value)) };
  }

  static #toPublicKey(value: unknown): JWK {
    const object = expectRecord(value, 'OIDC JWK');
    for (const field of PRIVATE_KEY_FIELDS) {
      if (object[field] !== undefined) {
        throw new Error('OIDC JWKS must not contain private or symmetric key material');
      }
    }
    const kty = requiredString(object.kty, 'OIDC JWK key type');
    if (!SUPPORTED_KEY_TYPES.has(kty)) {
      throw new Error('OIDC JWK key type is not supported');
    }
    const common = {
      kty,
      ...optionalString('kid', object.kid),
      ...optionalString('alg', object.alg),
      ...optionalSignatureUse(object.use),
      ...optionalStringArray('key_ops', object.key_ops),
      ...optionalStringArray('x5c', object.x5c),
      ...optionalString('x5t', object.x5t),
      ...optionalString('x5t#S256', object['x5t#S256']),
    };
    if (kty === 'RSA') {
      return {
        ...common,
        n: requiredString(object.n, 'OIDC RSA modulus'),
        e: requiredString(object.e, 'OIDC RSA exponent'),
      };
    }
    if (kty === 'EC') {
      return {
        ...common,
        crv: requiredString(object.crv, 'OIDC EC curve'),
        x: requiredString(object.x, 'OIDC EC x coordinate'),
        y: requiredString(object.y, 'OIDC EC y coordinate'),
      };
    }
    return {
      ...common,
      crv: requiredString(object.crv, 'OIDC OKP curve'),
      x: requiredString(object.x, 'OIDC OKP public key'),
    };
  }
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function requiredString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
}

function optionalString<Key extends string>(key: Key, value: unknown): Partial<Record<Key, string>> {
  return typeof value === 'string' && value.length > 0 ? { [key]: value } as Record<Key, string> : {};
}

function optionalStringArray<Key extends string>(key: Key, value: unknown): Partial<Record<Key, string[]>> {
  if (value === undefined) {
    return {};
  }
  if (!Array.isArray(value) || !value.every((entry) => typeof entry === 'string')) {
    throw new Error(`OIDC JWK ${key} must be a string array`);
  }
  return { [key]: value } as Record<Key, string[]>;
}

function optionalSignatureUse(value: unknown): { use?: string } {
  if (value === undefined) {
    return {};
  }
  if (value !== 'sig') {
    throw new Error('OIDC JWK use must be sig');
  }
  return { use: value };
}
