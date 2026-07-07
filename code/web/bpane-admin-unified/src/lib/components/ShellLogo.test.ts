import { afterEach, describe, expect, it } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ShellLogo from './ShellLogo.svelte';

afterEach(cleanupRenderedComponents);

describe('ShellLogo', () => {
  it('renders the compact BrowserPane brand link', () => {
    const target = renderComponent(ShellLogo);
    const logo = byTestId(target, 'admin-new-logo');

    expect(logo.tagName).toBe('A');
    expect(logo.getAttribute('href')).toBe('/admin-new/');
    expect(logo.getAttribute('aria-label')).toBe('BrowserPane admin home');
    expect(logo.textContent).toContain('BP');
    expect(logo.textContent).toContain('BrowserPane');
    expect(logo.textContent).toContain('Unified admin');
  });

  it('allows the shell subtitle to be overridden', () => {
    const target = renderComponent(ShellLogo, { subtitle: 'Control plane' });

    expect(byTestId(target, 'admin-new-logo').textContent).toContain('Control plane');
  });
});
