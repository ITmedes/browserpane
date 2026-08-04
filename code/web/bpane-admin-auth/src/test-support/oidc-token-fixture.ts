import {
  exportJWK,
  generateKeyPair,
  SignJWT,
  type CryptoKey,
  type JSONWebKeySet,
  type JWTPayload,
} from 'jose';

export type OidcTokenOverrides = {
  readonly aud?: string | string[];
  readonly iss?: string;
  readonly sub?: string;
  readonly iat?: number;
  readonly exp?: number;
  readonly nonce?: string | null;
  readonly azp?: string;
  readonly preferred_username?: string;
  readonly email?: string;
};

export class OidcTokenFixture {
  readonly #issuer: string;
  readonly #clientId: string;
  readonly #privateKey: CryptoKey;
  readonly #keySet: JSONWebKeySet;

  private constructor(issuer: string, clientId: string, privateKey: CryptoKey, keySet: JSONWebKeySet) {
    this.#issuer = issuer;
    this.#clientId = clientId;
    this.#privateKey = privateKey;
    this.#keySet = keySet;
  }

  static async create(issuer: string, clientId: string): Promise<OidcTokenFixture> {
    const pair = await generateKeyPair('RS256', { extractable: true });
    const publicJwk = await exportJWK(pair.publicKey);
    return new OidcTokenFixture(issuer, clientId, pair.privateKey, {
      keys: [{ ...publicJwk, alg: 'RS256', kid: 'test-key', use: 'sig' }],
    });
  }

  keySet(): JSONWebKeySet {
    return this.#keySet;
  }

  async sign(nowSeconds: number, overrides: OidcTokenOverrides = {}): Promise<string> {
    const payload: JWTPayload = {
      ...overrides,
      iss: overrides.iss ?? this.#issuer,
      aud: overrides.aud ?? this.#clientId,
      sub: overrides.sub ?? 'operator-subject',
      iat: overrides.iat ?? nowSeconds,
      exp: overrides.exp ?? nowSeconds + 300,
    };
    return await new SignJWT(payload)
      .setProtectedHeader({ alg: 'RS256', kid: 'test-key', typ: 'JWT' })
      .sign(this.#privateKey);
  }
}
