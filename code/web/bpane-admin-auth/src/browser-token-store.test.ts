import { describe, expect, it } from 'vitest';
import { BrowserTokenStore } from './browser-token-store';
import { MemoryStorage } from './test-support/memory-storage';

describe('BrowserTokenStore', () => {
  it('persists access and ID tokens without the refresh token', () => {
    const storage = new MemoryStorage();
    const store = new BrowserTokenStore(storage);

    store.saveTokens({
      access_token: 'access-token',
      id_token: 'id-token',
      refresh_token: 'refresh-token',
      expiresAtMs: 12_345,
    });

    const persisted = storage.getItem('bpane.admin.auth.tokens.v2');
    expect(persisted).toContain('access-token');
    expect(persisted).toContain('id-token');
    expect(persisted).not.toContain('refresh-token');
    expect(store.loadTokens()).not.toHaveProperty('refresh_token');
  });

  it('clears malformed and legacy token state', () => {
    const storage = new MemoryStorage();
    storage.setItem('bpane.admin.auth.tokens.v2', '{broken');
    storage.setItem('bpane.admin.auth.tokens.v1', '{"refresh_token":"legacy-secret"}');
    const store = new BrowserTokenStore(storage);

    expect(store.loadTokens()).toBeNull();
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
