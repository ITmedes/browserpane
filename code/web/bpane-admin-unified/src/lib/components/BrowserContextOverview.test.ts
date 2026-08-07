import { afterEach, describe, expect, it, vi } from 'vitest';

import type { BrowserContextResource } from '$lib/browser-contexts/browser-context-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import BrowserContextOverview from './BrowserContextOverview.svelte';

afterEach(cleanupRenderedComponents);

describe('BrowserContextOverview', () => {
  it('renders loading, error, and empty states', async () => {
    let target = renderComponent(BrowserContextOverview, { state: { status: 'loading' } });
    expect(byTestId(target, 'browser-contexts-loading').textContent).toContain('Loading browser contexts');

    await cleanupRenderedComponents();
    target = renderComponent(BrowserContextOverview, {
      state: { status: 'error', message: 'No active admin access token is available.' },
    });
    expect(byTestId(target, 'browser-contexts-error').textContent).toContain('Browser context catalog unavailable');

    await cleanupRenderedComponents();
    target = renderComponent(BrowserContextOverview, { state: { status: 'ready', contexts: [] } });
    expect(byTestId(target, 'browser-contexts-empty').textContent).toContain('Browser context catalog is empty');
  });

  it('renders metrics, catalog, and refresh action', () => {
    const onRefresh = vi.fn();
    const target = renderComponent(BrowserContextOverview, {
      state: {
        status: 'ready',
        contexts: [context()],
      },
      onRefresh,
    });

    expect(byTestId(target, 'browser-contexts-metric-total').textContent).toContain('1');
    expect(byTestId(target, 'browser-contexts-metric-ready').textContent).toContain('1');
    expect(byTestId(target, 'browser-contexts-list').textContent).toContain('Support baseline');
    expect(byTestId(target, 'browser-contexts-new-link').getAttribute('href')).toBe('/admin-new/browser-contexts/new');
    expect(byTestId(target, 'browser-contexts-import-link').getAttribute('href')).toBe(
      '/admin-new/browser-contexts/import',
    );

    byTestId(target, 'browser-contexts-refresh-button').click();
    expect(onRefresh).toHaveBeenCalledOnce();
  });
});

function context(): BrowserContextResource {
  return {
    id: 'context-1',
    project_id: null,
    project: null,
    name: 'Support baseline',
    description: 'Reusable support profile',
    labels: {},
    persistence_mode: 'reusable',
    retention_sec: 604800,
    retention_expires_at: null,
    max_profile_storage_bytes: 1073741824,
    state: 'ready',
    usage: {
      visible_session_count: 0,
      active_runtime_session_count: 0,
      active_runtime_session_id: null,
      profile_storage_bytes: 1048576,
      profile_storage_limit_exceeded: false,
    },
    created_at: '2026-06-18T09:00:00.000Z',
    updated_at: '2026-06-18T10:00:00.000Z',
    last_used_at: null,
    deleted_at: null,
  };
}
