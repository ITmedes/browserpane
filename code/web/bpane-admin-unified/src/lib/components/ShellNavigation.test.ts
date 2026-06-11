import { afterEach, describe, expect, it } from 'vitest';

import { allNavItems, navGroups, primaryNav } from '$lib/admin-navigation';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ShellNavigation from './ShellNavigation.svelte';

afterEach(cleanupRenderedComponents);

describe('ShellNavigation', () => {
  it('renders the desktop navigation from the shared nav model', () => {
    const target = renderComponent(ShellNavigation);
    const sideNav = byTestId(target, 'admin-new-side-nav');
    const links = Array.from(sideNav.querySelectorAll('a'));

    expect(links).toHaveLength(allNavItems.length);
    expect(links.map((link) => link.textContent?.trim())).toEqual(
      allNavItems.map((item) => item.label),
    );
    expect(links.map((link) => link.getAttribute('href'))).toEqual(
      allNavItems.map((item) => item.route),
    );
    expect(
      Array.from(sideNav.querySelectorAll('[data-testid="admin-new-nav-group"]')).map((group) =>
        group.textContent?.trim(),
      ),
    ).toEqual(navGroups.map((group) => group.group).filter(Boolean));
    expect(sideNav.textContent).toContain('Operate');
    expect(sideNav.textContent).toContain('Resources');
    expect(sideNav.textContent).toContain('Govern');
    expect(sideNav.textContent).toContain('Docs');
    expect(sideNav.textContent).toContain('Local stack');
    expect(sideNav.textContent).toContain('docker_pool');
  });

  it('renders a compact mobile navigation rail for the first primary items', () => {
    const target = renderComponent(ShellNavigation);
    const mobileNav = Array.from(target.querySelectorAll('nav[aria-label="Primary"]')).find(
      (nav) => nav.closest('aside') === null,
    );

    expect(mobileNav).toBeInstanceOf(HTMLElement);

    const links = Array.from((mobileNav as HTMLElement).querySelectorAll('a'));
    const expectedItems = primaryNav.slice(0, 6);

    expect(links).toHaveLength(expectedItems.length);
    expect(links.map((link) => link.getAttribute('aria-label'))).toEqual(
      expectedItems.map((item) => item.label),
    );
    expect(links.map((link) => link.getAttribute('href'))).toEqual(
      expectedItems.map((item) => item.route),
    );
  });
});
