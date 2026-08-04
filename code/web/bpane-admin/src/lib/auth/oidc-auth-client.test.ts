import { OidcAuthClientFactory as SharedOidcAuthClientFactory } from '@browserpane/admin-auth';
import { describe, expect, it } from 'vitest';
import { OidcAuthClientFactory } from './oidc-auth-client';

describe('compatibility admin auth adapter', () => {
  it('uses the shared BrowserPane auth implementation', () => {
    expect(OidcAuthClientFactory).toBe(SharedOidcAuthClientFactory);
  });
});
