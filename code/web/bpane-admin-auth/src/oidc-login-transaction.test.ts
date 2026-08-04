import { describe, expect, it } from 'vitest';
import { BrowserTokenStore } from './browser-token-store';
import { OidcLoginTransaction } from './oidc-login-transaction';
import { MemoryStorage } from './test-support/memory-storage';

describe('OidcLoginTransaction', () => {
  it('creates and consumes a nonce-bearing transaction exactly once', () => {
    const store = new BrowserTokenStore(new MemoryStorage());
    const transactions = new OidcLoginTransaction(store, () => 1_000);

    const created = transactions.create(new URL('http://localhost:8080/admin-new/?code=old'));

    expect(created.nonce).not.toBe(created.state);
    expect(created.redirectUri).toBe('http://localhost:8080/admin-new/');
    expect(transactions.consume()).toEqual(created);
    expect(() => transactions.consume()).toThrow('Missing OIDC login transaction');
  });

  it('rejects transactions older than ten minutes', () => {
    let nowMs = 1_000;
    const store = new BrowserTokenStore(new MemoryStorage());
    const transactions = new OidcLoginTransaction(store, () => nowMs);
    transactions.create(new URL('http://localhost:8080/admin/'));
    nowMs += 10 * 60_000 + 1;

    expect(() => transactions.consume()).toThrow('OIDC login transaction expired');
  });

  it('rejects a transaction created in the future', () => {
    let nowMs = 1_000;
    const store = new BrowserTokenStore(new MemoryStorage());
    const transactions = new OidcLoginTransaction(store, () => nowMs);
    transactions.create(new URL('http://localhost:8080/admin/'));
    nowMs = 999;

    expect(() => transactions.consume()).toThrow('OIDC login transaction expired');
  });
});
