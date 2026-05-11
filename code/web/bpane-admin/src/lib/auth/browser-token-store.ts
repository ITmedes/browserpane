import type { OidcTokenSet, PkceState } from './oidc-types';

const TOKEN_STORAGE_KEY = 'bpane.admin.auth.tokens.v1';
const PKCE_STORAGE_KEY = 'bpane.admin.auth.pkce.v1';

export type StorageLike = Pick<Storage, 'getItem' | 'removeItem' | 'setItem'>;

export class BrowserTokenStore {
  readonly #storage: StorageLike;

  constructor(storage: StorageLike) {
    this.#storage = storage;
  }

  loadTokens(): OidcTokenSet | null {
    const raw = this.#storage.getItem(TOKEN_STORAGE_KEY);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as Partial<OidcTokenSet>;
    if (typeof parsed.access_token !== 'string' || typeof parsed.expiresAtMs !== 'number') {
      return null;
    }
    return parsed as OidcTokenSet;
  }

  saveTokens(tokens: OidcTokenSet): void {
    this.#storage.setItem(TOKEN_STORAGE_KEY, JSON.stringify(tokens));
  }

  clearTokens(): void {
    this.#storage.removeItem(TOKEN_STORAGE_KEY);
  }

  loadPkceState(): PkceState | null {
    const raw = this.#storage.getItem(PKCE_STORAGE_KEY);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as Partial<PkceState>;
    if (
      typeof parsed.verifier !== 'string'
      || typeof parsed.state !== 'string'
      || typeof parsed.redirectUri !== 'string'
    ) {
      return null;
    }
    return parsed as PkceState;
  }

  savePkceState(state: PkceState): void {
    this.#storage.setItem(PKCE_STORAGE_KEY, JSON.stringify(state));
  }

  clearPkceState(): void {
    this.#storage.removeItem(PKCE_STORAGE_KEY);
  }
}
