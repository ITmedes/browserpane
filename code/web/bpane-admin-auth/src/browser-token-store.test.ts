import { describe, expect, it } from 'vitest';
import { BrowserTokenStore } from './browser-token-store';
import { MemoryStorage } from './test-support/memory-storage';

describe('BrowserTokenStore', () => {
  it('removes current and legacy token state without persisting credentials', () => {
    const storage = new MemoryStorage();
    storage.setItem('bpane.admin.auth.tokens.v2', '{"access_token":"current"}');
    storage.setItem('bpane.admin.auth.tokens.v1', '{"refresh_token":"legacy"}');

    new BrowserTokenStore(storage).clearTokens();

    expect(storage.getItem('bpane.admin.auth.tokens.v2')).toBeNull();
    expect(storage.getItem('bpane.admin.auth.tokens.v1')).toBeNull();
  });

  it('round-trips the short-lived login transaction without token material', () => {
    const storage = new MemoryStorage();
    const store = new BrowserTokenStore(storage);
    const transaction = {
      verifier: 'verifier',
      state: 'state',
      nonce: 'nonce',
      redirectUri: 'https://app.example.test/admin-new/',
      createdAtMs: 12_345,
    };

    store.savePkceState(transaction);

    expect(store.loadPkceState()).toEqual(transaction);
    expect(storage.getItem('bpane.admin.auth.tokens.v2')).toBeNull();
    expect(storage.getItem('bpane.admin.auth.tokens.v1')).toBeNull();
  });

  it('rejects incomplete login transaction state', () => {
    const storage = new MemoryStorage();
    storage.setItem('bpane.admin.auth.pkce.v2', JSON.stringify({ state: 'only-state' }));
    const store = new BrowserTokenStore(storage);

    expect(store.loadPkceState()).toBeNull();
    expect(storage.getItem('bpane.admin.auth.pkce.v2')).toBeNull();
  });
});
