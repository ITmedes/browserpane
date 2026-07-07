import { afterEach, describe, expect, it } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import Page from './+page.svelte';

afterEach(cleanupRenderedComponents);

describe('unified admin shell page', () => {
  it('composes the header controls, navigation, and empty workspace', () => {
    const target = renderComponent(Page);
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
