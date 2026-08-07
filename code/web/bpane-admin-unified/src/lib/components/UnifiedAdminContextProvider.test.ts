import { afterEach, describe, expect, it, vi } from 'vitest';
import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import UnifiedAdminContextProbe from './UnifiedAdminContextProbe.test.svelte';
import UnifiedAdminContextProviderFixture from './UnifiedAdminContextProvider.test.svelte';

afterEach(cleanupRenderedComponents);

describe('UnifiedAdminContextProvider', () => {
  it('provides the authenticated shell context to route descendants', () => {
    const target = renderComponent(UnifiedAdminContextProviderFixture, {
      authContext: context(),
    });

    expect(byTestId(target, 'unified-admin-context-probe').textContent).toContain('operator');
  });

  it('rejects route content rendered outside the authenticated shell', () => {
    expect(() => renderComponent(UnifiedAdminContextProbe)).toThrow(
      'Unified admin route rendered outside the authenticated shell.',
    );
  });
});

function context(): UnifiedAdminContext {
  return {
    auth: {
      configured: true,
      authenticated: true,
      username: 'operator',
      accessToken: 'token',
      claims: null,
    },
    authConfig: null,
    accessTokenProvider: async () => 'token',
    onAuthenticationFailure: vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}
