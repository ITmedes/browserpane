import type { OidcTokenSet, PkceState } from './oidc-types';

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

  loadTokens(): OidcTokenSet | null {
    const value = this.#readJson(TOKEN_STORAGE_KEY);
    if (!value || typeof value.access_token !== 'string' || typeof value.expiresAtMs !== 'number') {
      this.clearTokens();
      return null;
    }
    return {
      access_token: value.access_token,
      expiresAtMs: value.expiresAtMs,
      ...optionalStringProperty('token_type', value.token_type),
      ...optionalNumberProperty('expires_in', value.expires_in),
      ...optionalStringProperty('id_token', value.id_token),
    };
  }

  saveTokens(tokens: OidcTokenSet): void {
    const persisted = {
      access_token: tokens.access_token,
      expiresAtMs: tokens.expiresAtMs,
      ...optionalStringProperty('token_type', tokens.token_type),
      ...optionalNumberProperty('expires_in', tokens.expires_in),
      ...optionalStringProperty('id_token', tokens.id_token),
    };
    this.#storage.setItem(TOKEN_STORAGE_KEY, JSON.stringify(persisted));
    this.#storage.removeItem(LEGACY_TOKEN_STORAGE_KEY);
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

function optionalStringProperty<Key extends string>(key: Key, value: unknown): Partial<Record<Key, string>> {
  return typeof value === 'string' && value.length > 0 ? { [key]: value } as Record<Key, string> : {};
}

function optionalNumberProperty<Key extends string>(key: Key, value: unknown): Partial<Record<Key, number>> {
  return typeof value === 'number' && Number.isFinite(value) ? { [key]: value } as Record<Key, number> : {};
}
