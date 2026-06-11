import { describe, expect, it } from 'vitest';

import { allNavItems, navGroups } from './admin-navigation';

describe('admin navigation model', () => {
  it('keeps every route inside the unified admin route prefix', () => {
    for (const item of allNavItems) {
      expect(item.route).toMatch(/^\/admin-new(\/|$)/);
    }
  });

  it('has one active dashboard entry for the static shell', () => {
    const activeItems = allNavItems.filter((item) => item.active);

    expect(activeItems).toEqual([
      expect.objectContaining({
        id: 'dashboard',
        route: '/admin-new/',
      }),
    ]);
  });

  it('matches the grouped concept navigation structure', () => {
    expect(
      navGroups.map((group) => ({
        group: group.group,
        ids: group.items.map((item) => item.id),
      })),
    ).toEqual([
      { group: null, ids: ['dashboard'] },
      { group: 'Operate', ids: ['sessions', 'workflows', 'runs'] },
      { group: 'Resources', ids: ['projects', 'contexts', 'egress', 'workspaces'] },
      { group: 'Govern', ids: ['identity', 'api'] },
      { group: 'Docs', ids: ['memo', 'coverage'] },
    ]);
  });
});
