import * as oauth from 'oauth4webapi';
import { BrowserTokenStore } from './browser-token-store';
import type { PkceState } from './oidc-types';
import { PkceCodec } from './pkce-codec';

const PKCE_STATE_MAX_AGE_MS = 10 * 60_000;

export class OidcLoginTransaction {
  readonly #tokenStore: BrowserTokenStore;
  readonly #nowMs: () => number;

  constructor(tokenStore: BrowserTokenStore, nowMs: () => number) {
    this.#tokenStore = tokenStore;
    this.#nowMs = nowMs;
  }

  create(currentUrl: URL): PkceState {
    const transaction = {
      verifier: oauth.generateRandomCodeVerifier(),
      state: oauth.generateRandomState(),
      nonce: oauth.generateRandomNonce(),
      redirectUri: PkceCodec.buildRedirectUri(currentUrl),
      createdAtMs: this.#nowMs(),
    };
    this.#tokenStore.savePkceState(transaction);
    return transaction;
  }

  consume(): PkceState {
    const transaction = this.#tokenStore.loadPkceState();
    this.#tokenStore.clearPkceState();
    if (!transaction) {
      throw new Error('Missing OIDC login transaction');
    }
    const ageMs = this.#nowMs() - transaction.createdAtMs;
    if (ageMs < 0 || ageMs > PKCE_STATE_MAX_AGE_MS) {
      throw new Error('OIDC login transaction expired');
    }
    return transaction;
  }
}
