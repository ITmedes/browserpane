import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';

import type { BrowserContextResource } from '$lib/browser-contexts/browser-context-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import BrowserContextCatalogTable from './BrowserContextCatalogTable.svelte';

afterEach(cleanupRenderedComponents);

describe('BrowserContextCatalogTable', () => {
  it('renders context rows and filters by lens and search text', async () => {
    const target = renderComponent(BrowserContextCatalogTable, {
      contexts: [
        context({ id: 'ready-context', name: 'Ready context', projectName: 'Support' }),
        context({ id: 'deleted-context', name: 'Deleted context', state: 'deleted', storageOverLimit: true }),
      ],
    });

    expect(byTestId(target, 'browser-contexts-list-count').textContent).toContain('2 of 2');
    expect(target.querySelectorAll('[data-testid="browser-contexts-list-row"]')).toHaveLength(2);
    expect(byTestId(target, 'browser-contexts-list').textContent).toContain('Ready context');
    expect(byTestId(target, 'browser-contexts-list').textContent).toContain('Deleted context');
    expect((target.querySelector('[data-testid="browser-contexts-detail-link"]') as HTMLAnchorElement).href).toBe(
      'http://localhost:3000/admin-new/browser-contexts/ready-context',
    );

    byTestId(target, 'browser-contexts-lens-attention').click();
    await tick();

    expect(byTestId(target, 'browser-contexts-list-count').textContent).toContain('1 of 2');
    expect(target.querySelectorAll('[data-testid="browser-contexts-list-row"]')).toHaveLength(1);
    expect(byTestId(target, 'browser-contexts-list').textContent).toContain('Deleted context');

    byTestId(target, 'browser-contexts-lens-all').click();
    const search = byTestId(target, 'browser-contexts-search') as HTMLInputElement;
    search.value = 'missing';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();

    expect(byTestId(target, 'browser-contexts-filter-empty').textContent).toContain('No browser contexts match');
  });
});

function context(options: {
  readonly id: string;
  readonly name: string;
  readonly state?: BrowserContextResource['state'];
  readonly projectName?: string;
  readonly storageOverLimit?: boolean;
}): BrowserContextResource {
  return {
    id: options.id,
    project_id: options.projectName ? 'project-1' : null,
    project: options.projectName ? { id: 'project-1', name: options.projectName, state: 'active' } : null,
    name: options.name,
    description: `${options.name} profile`,
    labels: {},
    persistence_mode: 'reusable',
    retention_sec: 604800,
    retention_expires_at: null,
    max_profile_storage_bytes: 1073741824,
    state: options.state ?? 'ready',
    usage: {
      visible_session_count: 0,
      active_runtime_session_count: 0,
      active_runtime_session_id: null,
      profile_storage_bytes: 1048576,
      profile_storage_limit_exceeded: options.storageOverLimit ?? false,
    },
    created_at: '2026-06-18T09:00:00.000Z',
    updated_at: '2026-06-18T10:00:00.000Z',
    last_used_at: null,
    deleted_at: null,
  };
}
