import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';

import { sessionResource } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionCatalogTable from './SessionCatalogTable.svelte';

afterEach(cleanupRenderedComponents);

describe('SessionCatalogTable', () => {
  it('renders rows, filters by lens and search text, and links to details', async () => {
    const target = renderComponent(SessionCatalogTable, {
      sessions: [
        sessionResource({ id: 'active-session', totalClients: 1 }),
        sessionResource({ id: 'queued-session', state: 'queued', queued: true, totalClients: 0 }),
      ],
    });

    expect(byTestId(target, 'sessions-list-count').textContent).toContain('2 of 2');
    expect(target.querySelectorAll('[data-testid="sessions-list-row"]')).toHaveLength(2);
    expect((target.querySelector('[data-testid="sessions-detail-link"]') as HTMLAnchorElement).href).toBe(
      'http://localhost:3000/admin-new/sessions/active-session',
    );

    byTestId(target, 'sessions-lens-queued').click();
    await tick();

    expect(byTestId(target, 'sessions-list-count').textContent).toContain('1 of 2');
    expect(byTestId(target, 'sessions-list').textContent).toContain('queued-session');

    byTestId(target, 'sessions-lens-all').click();
    const search = byTestId(target, 'sessions-search') as HTMLInputElement;
    search.value = 'missing';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();

    expect(byTestId(target, 'sessions-filter-empty').textContent).toContain('No sessions match');
  });
});
