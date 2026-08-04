export type OidcTokenSet = {
  readonly access_token: string;
  readonly token_type?: string;
  readonly expires_in?: number;
  readonly refresh_token?: string;
  readonly id_token?: string;
  readonly expiresAtMs: number;
};

export type OidcClaims = {
  readonly sub: string;
  readonly preferred_username?: string;
  readonly email?: string;
};

export type OidcExchangeResult = {
  readonly tokens: OidcTokenSet;
  readonly claims?: OidcClaims;
};

export type PkceState = {
  readonly verifier: string;
  readonly state: string;
  readonly nonce: string;
  readonly redirectUri: string;
  readonly createdAtMs: number;
};

export type AuthSnapshot = {
  readonly configured: boolean;
  readonly authenticated: boolean;
  readonly username: string;
  readonly accessToken: string | null;
  readonly claims: OidcClaims | null;
};
