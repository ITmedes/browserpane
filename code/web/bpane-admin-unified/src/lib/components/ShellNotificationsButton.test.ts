import { afterEach, describe, expect, it } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ShellNotificationsButton from './ShellNotificationsButton.svelte';

afterEach(cleanupRenderedComponents);

describe('ShellNotificationsButton', () => {
  it('renders the notifications button with its accessible label', () => {
    const target = renderComponent(ShellNotificationsButton);
    const button = byTestId(target, 'admin-new-notifications');

    expect(button.tagName).toBe('BUTTON');
    expect(button.getAttribute('type')).toBe('button');
    expect(button.getAttribute('aria-label')).toBe('Notifications');
    expect(button.querySelector('svg')).not.toBeNull();
  });
});
