import { describe, expect, it } from 'vitest';

import { primaryNav, secondaryNav } from './admin-navigation';

describe('admin navigation model', () => {
  it('keeps every route inside the unified admin route prefix', () => {
    for (const item of [...primaryNav, ...secondaryNav]) {
      expect(item.route).toMatch(/^\/admin-new(\/|$)/);
    }
  });

  it('has one active dashboard entry for the static shell', () => {
    const activeItems = primaryNav.filter((item) => item.active);

    expect(activeItems).toEqual([
      expect.objectContaining({
        id: 'dashboard',
        route: '/admin-new/',
      }),
    ]);
  });

  it('covers the first migrated resource groups', () => {
    expect(primaryNav.map((item) => item.id)).toEqual([
      'dashboard',
      'sessions',
      'runs',
      'contexts',
      'egress',
      'projects',
      'workspaces',
    ]);
  });
});
