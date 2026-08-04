import type { OidcMetadata, OidcTokenSet } from './oidc-types';

export class OidcWireMapper {
  static toMetadata(payload: unknown): OidcMetadata {
    const object = expectRecord(payload, 'OIDC metadata');
    const endSessionEndpoint = optionalString(object.end_session_endpoint);
    return {
      issuer: requiredString(object.issuer, 'issuer'),
      authorization_endpoint: requiredString(object.authorization_endpoint, 'authorization endpoint'),
      token_endpoint: requiredString(object.token_endpoint, 'token endpoint'),
      jwks_uri: requiredString(object.jwks_uri, 'JWKS URI'),
      ...(endSessionEndpoint !== undefined ? { end_session_endpoint: endSessionEndpoint } : {}),
    };
  }

  static toTokenSet(payload: unknown, nowMs: number, previous?: OidcTokenSet): OidcTokenSet {
    const object = expectRecord(payload, 'OIDC token response');
    const expiresIn = optionalNumber(object.expires_in);
    const refreshToken = optionalString(object.refresh_token) ?? previous?.refresh_token;
    const idToken = optionalString(object.id_token) ?? previous?.id_token;
    const tokenType = optionalString(object.token_type) ?? previous?.token_type;
    return {
      access_token: requiredString(object.access_token, 'access token'),
      expiresAtMs: nowMs + Math.max(30, expiresIn ?? 60) * 1000,
      ...(expiresIn !== undefined ? { expires_in: expiresIn } : {}),
      ...(refreshToken !== undefined ? { refresh_token: refreshToken } : {}),
      ...(idToken !== undefined ? { id_token: idToken } : {}),
      ...(tokenType !== undefined ? { token_type: tokenType } : {}),
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

function optionalString(value: unknown): string | undefined {
  return typeof value === 'string' && value.length > 0 ? value : undefined;
}

function optionalNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}
