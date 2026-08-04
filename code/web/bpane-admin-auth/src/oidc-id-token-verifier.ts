import {
  createLocalJWKSet,
  errors,
  jwtVerify,
  type JSONWebKeySet,
  type JWTPayload,
} from 'jose';
import { JwksWireMapper } from './jwks-wire-mapper';
import { OidcEndpointClient } from './oidc-endpoint-client';
import type { OidcClaims } from './oidc-types';

const ALLOWED_ID_TOKEN_ALGORITHMS = [
  'RS256', 'RS384', 'RS512',
  'PS256', 'PS384', 'PS512',
  'ES256', 'ES384', 'ES512',
  'EdDSA',
] as const;

export class OidcIdTokenVerifier {
  readonly #endpoints: OidcEndpointClient;
  readonly #nowMs: () => number;
  #keySet: JSONWebKeySet | null = null;

  constructor(endpoints: OidcEndpointClient, nowMs: () => number) {
    this.#endpoints = endpoints;
    this.#nowMs = nowMs;
  }

  async verify(idToken: string, expectedNonce?: string): Promise<OidcClaims> {
    if (!idToken) {
      throw new Error('OIDC ID token is required');
    }
    const payload = await this.#verifyWithKeyRotation(idToken);
    this.#validateAuthorizedParty(payload);
    if (expectedNonce !== undefined && payload.nonce !== expectedNonce) {
      throw new Error('OIDC nonce mismatch');
    }
    return this.#toClaims(payload);
  }

  async #verifyWithKeyRotation(idToken: string): Promise<JWTPayload> {
    try {
      return await this.#verifySignature(idToken);
    } catch (error) {
      if (!(error instanceof errors.JWKSNoMatchingKey)
        && !(error instanceof errors.JWSSignatureVerificationFailed)) {
        throw error;
      }
      this.#keySet = null;
      return await this.#verifySignature(idToken);
    }
  }

  async #verifySignature(idToken: string): Promise<JWTPayload> {
    const metadata = await this.#endpoints.fetchMetadata();
    const { payload } = await jwtVerify(idToken, createLocalJWKSet(await this.#loadKeySet()), {
      algorithms: [...ALLOWED_ID_TOKEN_ALGORITHMS],
      audience: this.#endpoints.clientId(),
      issuer: metadata.issuer,
      currentDate: new Date(this.#nowMs()),
      clockTolerance: 5,
    });
    return payload;
  }

  async #loadKeySet(): Promise<JSONWebKeySet> {
    if (this.#keySet) {
      return this.#keySet;
    }
    const metadata = await this.#endpoints.fetchMetadata();
    const response = await this.#endpoints.fetchImpl()(metadata.jwks_uri, {
      cache: 'no-store',
      credentials: 'omit',
      redirect: 'error',
    });
    if (!response.ok) {
      throw new Error(`OIDC JWKS request failed with HTTP ${response.status}`);
    }
    this.#keySet = JwksWireMapper.toPublicKeySet(await response.json());
    return this.#keySet;
  }

  #validateAuthorizedParty(payload: JWTPayload): void {
    const clientId = this.#endpoints.clientId();
    const audiences = Array.isArray(payload.aud) ? payload.aud : [payload.aud];
    if (audiences.length > 1 && payload.azp !== clientId) {
      throw new Error('OIDC authorized party mismatch');
    }
    if (payload.azp !== undefined && payload.azp !== clientId) {
      throw new Error('OIDC authorized party mismatch');
    }
  }

  #toClaims(payload: JWTPayload): OidcClaims {
    if (typeof payload.sub !== 'string' || payload.sub.length === 0) {
      throw new Error('OIDC subject claim is required');
    }
    return {
      sub: payload.sub,
      ...optionalClaim('preferred_username', payload.preferred_username),
      ...optionalClaim('email', payload.email),
    };
  }
}

function optionalClaim<Key extends 'preferred_username' | 'email'>(
  key: Key,
  value: unknown,
): Partial<Record<Key, string>> {
  return typeof value === 'string' && value.length > 0 ? { [key]: value } as Record<Key, string> : {};
}
