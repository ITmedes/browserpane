import { mount, unmount } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';

import { primaryNav, secondaryNav } from '$lib/admin-navigation';
import ShellAccountButton from './ShellAccountButton.svelte';
import ShellLogo from './ShellLogo.svelte';
import ShellNavigation from './ShellNavigation.svelte';
import ShellNotificationsButton from './ShellNotificationsButton.svelte';
import ShellSearch from './ShellSearch.svelte';

type MountedComponent = {
  app: Record<string, any>;
  target: HTMLElement;
};

const mountedComponents: MountedComponent[] = [];

function render(component: any, props: Record<string, unknown> = {}) {
  const target = document.createElement('div');
  document.body.append(target);

  const app = mount(component, { target, props }) as Record<string, any>;
  mountedComponents.push({ app, target });

  return target;
}

function byTestId(target: ParentNode, testId: string) {
  const element = target.querySelector(`[data-testid="${testId}"]`);

  expect(element).toBeInstanceOf(HTMLElement);

  return element as HTMLElement;
}

afterEach(async () => {
  for (const { app, target } of mountedComponents.splice(0).reverse()) {
    await unmount(app);
    target.remove();
  }
});

describe('shell logo', () => {
  it('renders the compact BrowserPane brand link', () => {
    const target = render(ShellLogo);
    const logo = byTestId(target, 'admin-new-logo');

    expect(logo.tagName).toBe('A');
    expect(logo.getAttribute('href')).toBe('/admin-new/');
    expect(logo.getAttribute('aria-label')).toBe('BrowserPane admin home');
    expect(logo.textContent).toContain('BP');
    expect(logo.textContent).toContain('BrowserPane');
    expect(logo.textContent).toContain('Unified admin');
  });

  it('allows the shell subtitle to be overridden', () => {
    const target = render(ShellLogo, { subtitle: 'Control plane' });

    expect(byTestId(target, 'admin-new-logo').textContent).toContain('Control plane');
  });
});

describe('shell search', () => {
  it('renders the responsive header search control', () => {
    const target = render(ShellSearch);
    const search = byTestId(target, 'admin-new-search');

    expect(search.tagName).toBe('BUTTON');
    expect(search.getAttribute('type')).toBe('button');
    expect(search.classList.contains('ml-auto')).toBe(true);
    expect(search.classList.contains('hidden')).toBe(true);
    expect(search.classList.contains('lg:flex')).toBe(true);
    expect(search.textContent).toContain('Search sessions, workflows, resources');
    expect(search.querySelector('svg')).not.toBeNull();
  });

  it('allows the search placeholder to be overridden', () => {
    const target = render(ShellSearch, { placeholder: 'Search sessions' });

    expect(byTestId(target, 'admin-new-search').textContent).toContain('Search sessions');
  });
});

describe('shell header action buttons', () => {
  it('renders the notifications button with its accessible label', () => {
    const target = render(ShellNotificationsButton);
    const button = byTestId(target, 'admin-new-notifications');

    expect(button.tagName).toBe('BUTTON');
    expect(button.getAttribute('type')).toBe('button');
    expect(button.getAttribute('aria-label')).toBe('Notifications');
    expect(button.querySelector('svg')).not.toBeNull();
  });

  it('renders the account button with its accessible label', () => {
    const target = render(ShellAccountButton);
    const button = byTestId(target, 'admin-new-account');

    expect(button.tagName).toBe('BUTTON');
    expect(button.getAttribute('type')).toBe('button');
    expect(button.getAttribute('aria-label')).toBe('Account');
    expect(button.querySelector('svg')).not.toBeNull();
  });
});

describe('shell navigation', () => {
  it('renders the desktop navigation from the shared nav model', () => {
    const target = render(ShellNavigation);
    const sideNav = byTestId(target, 'admin-new-side-nav');
    const expectedItems = [...primaryNav, ...secondaryNav];
    const links = Array.from(sideNav.querySelectorAll('a'));

    expect(links).toHaveLength(expectedItems.length);
    expect(links.map((link) => link.textContent?.trim())).toEqual(
      expectedItems.map((item) => item.label),
    );
    expect(links.map((link) => link.getAttribute('href'))).toEqual(
      expectedItems.map((item) => item.route),
    );
    expect(sideNav.textContent).toContain('Govern');
    expect(sideNav.textContent).toContain('Local stack');
    expect(sideNav.textContent).toContain('docker_pool');
  });

  it('renders a compact mobile navigation rail for the first primary items', () => {
    const target = render(ShellNavigation);
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
