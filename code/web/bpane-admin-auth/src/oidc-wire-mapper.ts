import type { IDToken, TokenEndpointResponse } from 'oauth4webapi';
import type { OidcClaims, OidcTokenSet } from './oidc-types';

export class OidcWireMapper {
  static toTokenSet(payload: TokenEndpointResponse, nowMs: number, previous?: OidcTokenSet): OidcTokenSet {
    const refreshToken = payload.refresh_token ?? previous?.refresh_token;
    const idToken = payload.id_token ?? previous?.id_token;
    return {
      access_token: payload.access_token,
      expiresAtMs: nowMs + Math.max(0, payload.expires_in ?? 60) * 1000,
      ...(payload.expires_in !== undefined ? { expires_in: payload.expires_in } : {}),
      ...(refreshToken !== undefined ? { refresh_token: refreshToken } : {}),
      ...(idToken !== undefined ? { id_token: idToken } : {}),
      token_type: payload.token_type,
    };
  }

  static toClaims(payload: IDToken): OidcClaims {
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
