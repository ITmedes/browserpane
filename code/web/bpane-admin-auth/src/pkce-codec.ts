export class PkceCodec {
  static buildRedirectUri(currentUrl: URL): string {
    const clean = new URL(currentUrl.toString());
    for (const key of ['code', 'state', 'session_state', 'error', 'error_description']) {
      clean.searchParams.delete(key);
    }
    return clean.toString();
  }

}
