import { describe, expect, it } from 'vitest';
import { PkceCodec } from './pkce-codec';

describe('PkceCodec', () => {
  it('removes all OIDC response parameters while preserving application state', () => {
    const currentUrl = new URL('https://app.example.test/admin-new/sessions/session-1/live');
    currentUrl.searchParams.set('panel', 'metrics');
    currentUrl.searchParams.append('iss', 'https://identity.example.test/realms/browserpane');
    currentUrl.searchParams.append('iss', 'https://identity.example.test/realms/browserpane');
    currentUrl.searchParams.set('code', 'authorization-code');
    currentUrl.searchParams.set('state', 'login-state');
    currentUrl.searchParams.set('session_state', 'provider-session');
    currentUrl.searchParams.set('error', 'access_denied');
    currentUrl.searchParams.set('error_description', 'Denied');
    currentUrl.searchParams.set('error_uri', 'https://identity.example.test/errors/access-denied');

    expect(PkceCodec.buildRedirectUri(currentUrl)).toBe(
      'https://app.example.test/admin-new/sessions/session-1/live?panel=metrics',
    );
  });
});
