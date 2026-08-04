export class PkceCodec {
  static randomString(cryptoImpl: Crypto, byteLength: number): string {
    if (!Number.isInteger(byteLength) || byteLength < 16 || byteLength > 128) {
      throw new Error('OIDC random input length is invalid');
    }
    const bytes = new Uint8Array(byteLength);
    cryptoImpl.getRandomValues(bytes);
    return this.base64UrlEncode(bytes);
  }

  static buildRedirectUri(currentUrl: URL): string {
    const clean = new URL(currentUrl.toString());
    for (const key of ['code', 'state', 'session_state', 'error', 'error_description']) {
      clean.searchParams.delete(key);
    }
    return clean.toString();
  }

  static async sha256Base64Url(cryptoImpl: Crypto, input: string): Promise<string> {
    const bytes = new TextEncoder().encode(input);
    const hash = await cryptoImpl.subtle.digest('SHA-256', bytes);
    return this.base64UrlEncode(new Uint8Array(hash));
  }

  static base64UrlEncode(bytes: Uint8Array): string {
    let binary = '';
    for (const value of bytes) {
      binary += String.fromCharCode(value);
    }
    return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
  }
}
