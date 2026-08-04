import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import UnifiedAdminShell from './UnifiedAdminShell.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('UnifiedAdminShell authentication', () => {
  it('renders a bounded unauthenticated state when OIDC is not configured', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('', { status: 404 })));

    const target = renderComponent(UnifiedAdminShell);

    await vi.waitFor(() => {
      expect(byTestId(target, 'admin-new-auth-unconfigured').textContent).toContain('OIDC is not configured');
    });
  });
});
