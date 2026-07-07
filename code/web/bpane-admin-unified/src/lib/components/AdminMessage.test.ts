import { afterEach, describe, expect, it } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import AdminMessage from './AdminMessage.svelte';

afterEach(cleanupRenderedComponents);

describe('AdminMessage', () => {
  it('renders accessible alert content with itemized messages', () => {
    const target = renderComponent(AdminMessage, {
      tone: 'error',
      title: 'Validation failed',
      items: ['Project name is required.'],
      testId: 'admin-message-test',
    });

    const message = byTestId(target, 'admin-message-test');
    expect(message.getAttribute('role')).toBe('alert');
    expect(message.textContent).toContain('Validation failed');
    expect(message.textContent).toContain('Project name is required.');
  });
});
