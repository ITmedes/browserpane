import type { OidcClaims } from './oidc-types';

export class PkceCodec {
  static buildRedirectUri(currentUrl: URL): string {
    const url = new URL(currentUrl);
    url.searchParams.delete('code');
    url.searchParams.delete('state');
    url.searchParams.delete('session_state');
    url.searchParams.delete('iss');
    url.hash = '';
    return `${url.origin}${url.pathname}${url.search}`;
  }

  static randomString(cryptoImpl: Crypto, byteLength: number): string {
    const bytes = new Uint8Array(byteLength);
    cryptoImpl.getRandomValues(bytes);
    return this.base64UrlEncode(bytes);
  }

  static async sha256Base64Url(cryptoImpl: Crypto, input: string): Promise<string> {
    const bytes = new TextEncoder().encode(input);
    const hash = await cryptoImpl.subtle.digest('SHA-256', bytes);
    return this.base64UrlEncode(new Uint8Array(hash));
  }

  static parseJwtClaims(token: string | undefined): OidcClaims | null {
    if (!token) {
      return null;
    }
    const parts = token.split('.');
    const payload = parts[1];
    if (!payload) {
      return null;
    }
    try {
      const decoded = atob(toBase64(payload));
      return JSON.parse(decoded) as OidcClaims;
    } catch {
      return null;
    }
  }

  static base64UrlEncode(bytes: Uint8Array): string {
    let binary = '';
    for (const value of bytes) {
      binary += String.fromCharCode(value);
    }
    return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
  }
}

function toBase64(value: string): string {
  const base64 = value.replace(/-/g, '+').replace(/_/g, '/');
  return base64.padEnd(Math.ceil(base64.length / 4) * 4, '=');
}
