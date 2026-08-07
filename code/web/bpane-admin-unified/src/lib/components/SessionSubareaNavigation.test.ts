import { afterEach, describe, expect, it } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionSubareaNavigation from './SessionSubareaNavigation.svelte';

afterEach(cleanupRenderedComponents);

describe('SessionSubareaNavigation', () => {
  it('renders available routes and marks the active route', () => {
    const target = renderComponent(SessionSubareaNavigation, {
      sessionId: 'session/one',
      activeId: 'live',
    });

    expect(byTestId(target, 'session-subarea-overview').getAttribute('href'))
      .toBe('/admin-new/sessions/session%2Fone');
    expect(byTestId(target, 'session-subarea-live').getAttribute('href'))
      .toBe('/admin-new/sessions/session%2Fone/live');
    expect(byTestId(target, 'session-subarea-live').getAttribute('aria-current')).toBe('page');
    expect(target.querySelector('[data-testid="session-subarea-files"]')).toBeNull();
  });

  it('reveals additional subareas only when their routes are available', () => {
    const target = renderComponent(SessionSubareaNavigation, {
      sessionId: 'session-1',
      activeId: 'files',
      availableIds: ['overview', 'live', 'files'],
    });

    expect(byTestId(target, 'session-subarea-files').getAttribute('href'))
      .toBe('/admin-new/sessions/session-1/files');
    expect(byTestId(target, 'session-subarea-files').getAttribute('aria-current')).toBe('page');
  });
});
