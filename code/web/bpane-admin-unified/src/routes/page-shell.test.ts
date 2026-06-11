import { mount, unmount } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';

import Page from './+page.svelte';

type MountedComponent = {
  app: Record<string, any>;
  target: HTMLElement;
};

const mountedComponents: MountedComponent[] = [];

function renderPage() {
  const target = document.createElement('div');
  document.body.append(target);

  const app = mount(Page, { target }) as Record<string, any>;
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

describe('unified admin shell page', () => {
  it('composes the header controls, navigation, and empty workspace', () => {
    const target = renderPage();
    const shell = byTestId(target, 'admin-new-shell');
    const header = byTestId(target, 'admin-new-header');

    expect(shell.tagName).toBe('MAIN');
    expect(
      Array.from(header.querySelectorAll('[data-testid]')).map((element) =>
        element.getAttribute('data-testid'),
      ),
    ).toEqual([
      'admin-new-logo',
      'admin-new-search',
      'admin-new-notifications',
      'admin-new-account',
    ]);
    expect(header.textContent).not.toContain('Dashboard');
    expect(byTestId(target, 'admin-new-side-nav')).toBeInstanceOf(HTMLElement);
    expect(target.querySelector('section[aria-label="Workspace"]')).toBeInstanceOf(HTMLElement);
  });
});
