import { afterEach, describe, expect, it } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ShellSearch from './ShellSearch.svelte';

afterEach(cleanupRenderedComponents);

describe('ShellSearch', () => {
  it('renders the responsive header search control', () => {
    const target = renderComponent(ShellSearch);
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
    const target = renderComponent(ShellSearch, { placeholder: 'Search sessions' });

    expect(byTestId(target, 'admin-new-search').textContent).toContain('Search sessions');
  });
});
