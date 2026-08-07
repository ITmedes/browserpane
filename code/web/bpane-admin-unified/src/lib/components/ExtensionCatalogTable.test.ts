import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';

import type { ExtensionDefinitionResource } from '$lib/extensions/extension-types';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ExtensionCatalogTable from './ExtensionCatalogTable.svelte';

afterEach(cleanupRenderedComponents);

describe('ExtensionCatalogTable', () => {
  it('filters by lifecycle state and search text', async () => {
    const target = renderComponent(ExtensionCatalogTable, {
      extensions: [extension('enabled-extension', true, '1.0.0'), extension('disabled-extension', false, null)],
    });

    expect(target.querySelectorAll('[data-testid="extensions-list-row"]')).toHaveLength(2);
    byTestId(target, 'extensions-lens-disabled').click();
    await tick();
    expect(target.querySelectorAll('[data-testid="extensions-list-row"]')).toHaveLength(1);
    expect(target.textContent).toContain('disabled-extension');

    byTestId(target, 'extensions-lens-all').click();
    await input(target, 'extensions-search', '1.0.0');
    expect(target.querySelectorAll('[data-testid="extensions-list-row"]')).toHaveLength(1);
    expect(target.textContent).toContain('enabled-extension');
  });

  it('links each row to a direct detail route', () => {
    const target = renderComponent(ExtensionCatalogTable, { extensions: [extension('extension/one', true, '1.0.0')] });
    expect((byTestId(target, 'extensions-detail-link') as HTMLAnchorElement).getAttribute('href'))
      .toBe('/admin-new/extensions/extension%2Fone');
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}

function extension(id: string, enabled: boolean, version: string | null): ExtensionDefinitionResource {
  return {
    id,
    name: id,
    description: 'Approved extension',
    enabled,
    latest_version: version,
    labels: { team: 'platform' },
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
