import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ExtensionDefinitionResource } from '$lib/extensions/extension-types';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ExtensionOverview from './ExtensionOverview.svelte';

afterEach(cleanupRenderedComponents);

describe('ExtensionOverview', () => {
  it('renders loading, error, empty, and ready states', async () => {
    let target = renderComponent(ExtensionOverview, { state: { status: 'loading' } });
    expect(byTestId(target, 'extensions-loading').textContent).toContain('Loading approved extensions');

    await cleanupRenderedComponents();
    target = renderComponent(ExtensionOverview, { state: { status: 'error', message: 'Unavailable.' } });
    expect(byTestId(target, 'extensions-error').textContent).toContain('Unavailable');

    await cleanupRenderedComponents();
    target = renderComponent(ExtensionOverview, { state: { status: 'ready', extensions: [] } });
    expect(byTestId(target, 'extensions-empty').textContent).toContain('catalog is empty');

    await cleanupRenderedComponents();
    target = renderComponent(ExtensionOverview, { state: { status: 'ready', extensions: [extension()] } });
    expect(byTestId(target, 'extensions-metric-total').textContent).toContain('1');
    expect(byTestId(target, 'extensions-list').textContent).toContain('Login helper');
  });

  it('delegates refresh', () => {
    const onRefresh = vi.fn();
    const target = renderComponent(ExtensionOverview, { state: { status: 'ready', extensions: [extension()] }, onRefresh });
    byTestId(target, 'extensions-refresh-button').click();
    expect(onRefresh).toHaveBeenCalledOnce();
  });
});

function extension(): ExtensionDefinitionResource {
  return {
    id: 'extension-1',
    name: 'Login helper',
    description: null,
    enabled: true,
    latest_version: '1.0.0',
    labels: {},
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
