import type { PkceState } from './oidc-types';

const TOKEN_STORAGE_KEY = 'bpane.admin.auth.tokens.v2';
const PKCE_STORAGE_KEY = 'bpane.admin.auth.pkce.v2';
const LEGACY_TOKEN_STORAGE_KEY = 'bpane.admin.auth.tokens.v1';
const LEGACY_PKCE_STORAGE_KEY = 'bpane.admin.auth.pkce.v1';

export type StorageLike = Pick<Storage, 'getItem' | 'removeItem' | 'setItem'>;

export class BrowserTokenStore {
  readonly #storage: StorageLike;

  constructor(storage: StorageLike) {
    this.#storage = storage;
  }

  clearTokens(): void {
    this.#storage.removeItem(TOKEN_STORAGE_KEY);
    this.#storage.removeItem(LEGACY_TOKEN_STORAGE_KEY);
  }

  loadPkceState(): PkceState | null {
    const value = this.#readJson(PKCE_STORAGE_KEY);
    if (
      !value
      || typeof value.verifier !== 'string'
      || typeof value.state !== 'string'
      || typeof value.nonce !== 'string'
      || typeof value.redirectUri !== 'string'
      || typeof value.createdAtMs !== 'number'
    ) {
      this.clearPkceState();
      return null;
    }
    return {
      verifier: value.verifier,
      state: value.state,
      nonce: value.nonce,
      redirectUri: value.redirectUri,
      createdAtMs: value.createdAtMs,
    };
  }

  savePkceState(state: PkceState): void {
    this.#storage.setItem(PKCE_STORAGE_KEY, JSON.stringify(state));
    this.#storage.removeItem(LEGACY_PKCE_STORAGE_KEY);
  }

  clearPkceState(): void {
    this.#storage.removeItem(PKCE_STORAGE_KEY);
    this.#storage.removeItem(LEGACY_PKCE_STORAGE_KEY);
  }

  #readJson(key: string): Record<string, unknown> | null {
    const raw = this.#storage.getItem(key);
    if (!raw) {
      return null;
    }
    try {
      const value: unknown = JSON.parse(raw);
      return value && typeof value === 'object' && !Array.isArray(value)
        ? value as Record<string, unknown>
        : null;
    } catch {
      return null;
    }
  }
}
