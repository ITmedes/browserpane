import { existsSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { allNavItems, navGroups } from './admin-navigation';

describe('admin navigation model', () => {
  it('uses unique ids and routes', () => {
    expect(new Set(allNavItems.map((item) => item.id)).size).toBe(allNavItems.length);
    expect(new Set(allNavItems.map((item) => item.route)).size).toBe(allNavItems.length);
  });

  it('only advertises route-backed pages', () => {
    const routesDirectory = path.resolve(import.meta.dirname, '../routes');

    for (const item of allNavItems) {
      const relativeRoute = item.route.replace(/^\/admin-new\/?/, '');
      const pagePath = path.join(routesDirectory, relativeRoute, '+page.svelte');

      expect(existsSync(pagePath), `${item.label} route ${item.route}`).toBe(true);
    }
  });

  it('keeps every route inside the unified admin route prefix', () => {
    for (const item of allNavItems) {
      expect(item.route).toMatch(/^\/admin-new(\/|$)/);
    }
  });

  it('routes browser contexts to the API-aligned resource path', () => {
    expect(allNavItems.find((item) => item.id === 'contexts')).toMatchObject({
      route: '/admin-new/browser-contexts',
    });
  });

  it('routes file workspaces to the nested file resource path', () => {
    expect(allNavItems.find((item) => item.id === 'workspaces')).toMatchObject({
      route: '/admin-new/files/workspaces',
    });
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
      { group: 'Operate', ids: ['sessions', 'recordings', 'workflows', 'runs'] },
      {
        group: 'Resources',
        ids: ['projects', 'contexts', 'egress', 'workspaces', 'extensions', 'credentials'],
      },
      { group: 'Govern', ids: ['identity', 'events', 'api'] },
      { group: 'Docs', ids: ['memo', 'coverage'] },
    ]);
  });
});
