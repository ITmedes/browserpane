import { afterEach, describe, expect, it } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ShellAccountButton from './ShellAccountButton.svelte';

afterEach(cleanupRenderedComponents);

describe('ShellAccountButton', () => {
  it('renders the account button with its accessible label', () => {
    const target = renderComponent(ShellAccountButton);
    const button = byTestId(target, 'admin-new-account');

    expect(button.tagName).toBe('BUTTON');
    expect(button.getAttribute('type')).toBe('button');
    expect(button.getAttribute('aria-label')).toBe('Account');
    expect(button.querySelector('svg')).not.toBeNull();
  });
});
