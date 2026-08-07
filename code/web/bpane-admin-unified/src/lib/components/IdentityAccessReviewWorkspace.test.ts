import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { identityAccessReviewFixture } from '$lib/test-utils/identity-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import IdentityAccessReviewWorkspace from './IdentityAccessReviewWorkspace.svelte';

afterEach(cleanupRenderedComponents);

describe('IdentityAccessReviewWorkspace', () => {
  it('switches between review, principal, and mapping areas without losing route context', async () => {
    const target = renderComponent(IdentityAccessReviewWorkspace, {
      state: { status: 'ready', review: identityAccessReviewFixture() },
      actionState: { status: 'idle' },
      onRefresh: vi.fn(),
      onCreateServicePrincipal: vi.fn(),
      onUpdateServicePrincipal: vi.fn(),
      onCreateIdentityMapping: vi.fn(),
      onUpdateIdentityMapping: vi.fn(),
    });

    expect(byTestId(target, 'identity-review-summary')).toBeTruthy();
    byTestId(target, 'identity-area-service-principals').click();
    await tick();
    expect(byTestId(target, 'service-principal-catalog')).toBeTruthy();
    expect(byTestId(target, 'identity-area-service-principals').getAttribute('aria-pressed')).toBe('true');

    byTestId(target, 'identity-area-mappings').click();
    await tick();
    expect(byTestId(target, 'identity-mapping-catalog')).toBeTruthy();
    expect(byTestId(target, 'identity-area-mappings').textContent).toContain('1');
  });

  it('renders stable loading, error, running, success, and retry controls', () => {
    const onRefresh = vi.fn();
    const loading = renderComponent(IdentityAccessReviewWorkspace, {
      state: { status: 'loading' },
      actionState: { status: 'running', label: 'Refreshing...' },
      onRefresh,
    });
    expect(byTestId(loading, 'identity-loading').textContent).toContain('Loading identity');
    expect(byTestId(loading, 'identity-action-running').textContent).toContain('Refreshing');
    expect((byTestId(loading, 'identity-refresh') as HTMLButtonElement).disabled).toBe(true);

    const failed = renderComponent(IdentityAccessReviewWorkspace, {
      state: { status: 'error', message: 'Permission denied.' },
      actionState: { status: 'error', message: 'Refresh failed.' },
      onRefresh,
    });
    expect(byTestId(failed, 'identity-load-error').textContent).toContain('Permission denied');
    expect(byTestId(failed, 'identity-action-error').textContent).toContain('Refresh failed');
    byTestId(failed, 'identity-refresh').click();
    expect(onRefresh).toHaveBeenCalledOnce();
  });
});
