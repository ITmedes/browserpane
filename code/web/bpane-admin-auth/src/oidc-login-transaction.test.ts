import { webcrypto } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { BrowserTokenStore } from './browser-token-store';
import { OidcLoginTransaction } from './oidc-login-transaction';
import { MemoryStorage } from './test-support/memory-storage';

describe('OidcLoginTransaction', () => {
  it('creates and consumes a nonce-bearing transaction exactly once', () => {
    const store = new BrowserTokenStore(new MemoryStorage());
    const transactions = new OidcLoginTransaction(store, webcrypto as Crypto, () => 1_000);

    const created = transactions.create(new URL('http://localhost:8080/admin-new/?code=old'));

    expect(created.nonce).not.toBe(created.state);
    expect(created.redirectUri).toBe('http://localhost:8080/admin-new/');
    expect(transactions.consume(created.state)).toEqual(created);
    expect(() => transactions.consume(created.state)).toThrow('Missing OIDC login transaction');
  });

  it('rejects a mismatched state and clears the transaction', () => {
    const store = new BrowserTokenStore(new MemoryStorage());
    const transactions = new OidcLoginTransaction(store, webcrypto as Crypto, () => 1_000);
    transactions.create(new URL('http://localhost:8080/admin/'));

    expect(() => transactions.consume('wrong-state')).toThrow('OIDC state mismatch');
    expect(store.loadPkceState()).toBeNull();
  });

  it('rejects transactions older than ten minutes', () => {
    let nowMs = 1_000;
    const store = new BrowserTokenStore(new MemoryStorage());
    const transactions = new OidcLoginTransaction(store, webcrypto as Crypto, () => nowMs);
    const created = transactions.create(new URL('http://localhost:8080/admin/'));
    nowMs += 10 * 60_000 + 1;

    expect(() => transactions.consume(created.state)).toThrow('OIDC login transaction expired');
  });
});
