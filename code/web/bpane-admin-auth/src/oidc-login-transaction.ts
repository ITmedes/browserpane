import { BrowserTokenStore } from './browser-token-store';
import type { PkceState } from './oidc-types';
import { PkceCodec } from './pkce-codec';

const PKCE_STATE_MAX_AGE_MS = 10 * 60_000;

export class OidcLoginTransaction {
  readonly #tokenStore: BrowserTokenStore;
  readonly #cryptoImpl: Crypto;
  readonly #nowMs: () => number;

  constructor(tokenStore: BrowserTokenStore, cryptoImpl: Crypto, nowMs: () => number) {
    this.#tokenStore = tokenStore;
    this.#cryptoImpl = cryptoImpl;
    this.#nowMs = nowMs;
  }

  create(currentUrl: URL): PkceState {
    const transaction = {
      verifier: PkceCodec.randomString(this.#cryptoImpl, 48),
      state: PkceCodec.randomString(this.#cryptoImpl, 24),
      nonce: PkceCodec.randomString(this.#cryptoImpl, 24),
      redirectUri: PkceCodec.buildRedirectUri(currentUrl),
      createdAtMs: this.#nowMs(),
    };
    this.#tokenStore.savePkceState(transaction);
    return transaction;
  }

  consume(returnedState: string | null): PkceState {
    const transaction = this.#tokenStore.loadPkceState();
    this.#tokenStore.clearPkceState();
    if (!transaction) {
      throw new Error('Missing OIDC login transaction');
    }
    if (transaction.state !== returnedState) {
      throw new Error('OIDC state mismatch');
    }
    const ageMs = this.#nowMs() - transaction.createdAtMs;
    if (ageMs < 0 || ageMs > PKCE_STATE_MAX_AGE_MS) {
      throw new Error('OIDC login transaction expired');
    }
    return transaction;
  }
}
