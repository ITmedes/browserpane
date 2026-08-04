import { beforeAll, describe, expect, it, vi } from 'vitest';
import type { AuthConfig } from './auth-config';
import { OidcEndpointClient } from './oidc-endpoint-client';
import { OidcIdTokenVerifier } from './oidc-id-token-verifier';
import { OidcTokenFixture } from './test-support/oidc-token-fixture';

const ISSUER = 'https://identity.example.test/realms/browserpane';
const CLIENT_ID = 'bpane-web';
const NOW_SECONDS = 1_700_000_000;
const CONFIG: AuthConfig = { mode: 'oidc', issuer: ISSUER, clientId: CLIENT_ID, scope: 'openid' };
const METADATA = {
  issuer: ISSUER,
  authorization_endpoint: `${ISSUER}/authorize`,
  token_endpoint: `${ISSUER}/token`,
  jwks_uri: `${ISSUER}/jwks`,
};

describe('OidcIdTokenVerifier', () => {
  let fixture: OidcTokenFixture;

  beforeAll(async () => {
    fixture = await OidcTokenFixture.create(ISSUER, CLIENT_ID);
  });

  it('accepts a signed token with matching issuer, audience, nonce, and party', async () => {
    const verifier = newVerifier(fixture);
    const token = await fixture.sign(NOW_SECONDS, {
      nonce: 'expected-nonce',
      preferred_username: 'operator',
      aud: [CLIENT_ID, 'secondary-audience'],
      azp: CLIENT_ID,
    });

    await expect(verifier.verify(token, 'expected-nonce')).resolves.toEqual({
      sub: 'operator-subject',
      preferred_username: 'operator',
    });
  });

  it.each([
    ['issuer', { iss: 'https://attacker.example.test' }],
    ['audience', { aud: 'other-client' }],
    ['expiry', { exp: NOW_SECONDS - 30 }],
    ['authorized party', { aud: [CLIENT_ID, 'secondary'], azp: 'other-client' }],
  ])('rejects a token with the wrong %s', async (_label, overrides) => {
    const verifier = newVerifier(fixture);
    const token = await fixture.sign(NOW_SECONDS, overrides);

    await expect(verifier.verify(token)).rejects.toThrow();
  });

  it('rejects a nonce mismatch', async () => {
    const verifier = newVerifier(fixture);
    const token = await fixture.sign(NOW_SECONDS, { nonce: 'issued-nonce' });

    await expect(verifier.verify(token, 'different-nonce')).rejects.toThrow('OIDC nonce mismatch');
  });

  it('rejects a token signed by an unknown key', async () => {
    const otherFixture = await OidcTokenFixture.create(ISSUER, CLIENT_ID);
    const verifier = newVerifier(fixture);
    const token = await otherFixture.sign(NOW_SECONDS);

    await expect(verifier.verify(token)).rejects.toThrow();
  });

  it('refreshes cached keys once when the provider rotates a signing key with the same id', async () => {
    const rotatedFixture = await OidcTokenFixture.create(ISSUER, CLIENT_ID);
    let activeFixture = fixture;
    let jwksRequests = 0;
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      if (String(input).endsWith('/jwks')) {
        jwksRequests += 1;
        return jsonResponse(activeFixture.keySet());
      }
      return jsonResponse(METADATA);
    });
    const verifier = new OidcIdTokenVerifier(
      new OidcEndpointClient(CONFIG, fetchImpl),
      () => NOW_SECONDS * 1000,
    );

    await verifier.verify(await fixture.sign(NOW_SECONDS));
    activeFixture = rotatedFixture;

    await expect(verifier.verify(await rotatedFixture.sign(NOW_SECONDS))).resolves.toEqual({
      sub: 'operator-subject',
    });
    expect(jwksRequests).toBe(2);
  });

  it('rejects discovery metadata from another issuer before fetching keys', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse({
      ...METADATA,
      issuer: 'https://attacker.example.test',
    }));
    const endpoints = new OidcEndpointClient(CONFIG, fetchImpl);
    const verifier = new OidcIdTokenVerifier(endpoints, () => NOW_SECONDS * 1000);
    const token = await fixture.sign(NOW_SECONDS);

    await expect(verifier.verify(token)).rejects.toThrow('OIDC discovery issuer mismatch');
    expect(fetchImpl).toHaveBeenCalledTimes(1);
  });
});

function newVerifier(fixture: OidcTokenFixture): OidcIdTokenVerifier {
  const fetchImpl = vi.fn<typeof fetch>(async (input) => {
    return String(input).endsWith('/jwks')
      ? jsonResponse(fixture.keySet())
      : jsonResponse(METADATA);
  });
  const endpoints = new OidcEndpointClient(CONFIG, fetchImpl);
  return new OidcIdTokenVerifier(endpoints, () => NOW_SECONDS * 1000);
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
