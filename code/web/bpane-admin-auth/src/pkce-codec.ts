export class PkceCodec {
  static buildRedirectUri(currentUrl: URL): string {
    const clean = new URL(currentUrl.toString());
    for (const key of [
      'code',
      'state',
      'session_state',
      'iss',
      'error',
      'error_description',
      'error_uri',
    ]) {
      clean.searchParams.delete(key);
    }
    return clean.toString();
  }
}
