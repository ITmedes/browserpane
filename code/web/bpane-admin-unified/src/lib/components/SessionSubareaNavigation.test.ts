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

  it('links the automation area after that route is enabled', () => {
    const target = renderComponent(SessionSubareaNavigation, {
      sessionId: 'session-1',
      activeId: 'automation',
      availableIds: ['overview', 'live', 'automation'],
    });

    expect(byTestId(target, 'session-subarea-automation').getAttribute('href'))
      .toBe('/admin-new/sessions/session-1/automation');
    expect(byTestId(target, 'session-subarea-automation').getAttribute('aria-current')).toBe('page');
  });

  it('links the policy area after that route is enabled', () => {
    const target = renderComponent(SessionSubareaNavigation, {
      sessionId: 'session-1',
      activeId: 'policy',
      availableIds: ['overview', 'live', 'automation', 'policy'],
    });

    expect(byTestId(target, 'session-subarea-policy').getAttribute('href'))
      .toBe('/admin-new/sessions/session-1/policy');
    expect(byTestId(target, 'session-subarea-policy').getAttribute('aria-current')).toBe('page');
  });

  it('links the recordings area after that route is enabled', () => {
    const target = renderComponent(SessionSubareaNavigation, {
      sessionId: 'session-1',
      activeId: 'recordings',
      availableIds: ['overview', 'live', 'files', 'recordings'],
    });

    expect(byTestId(target, 'session-subarea-recordings').getAttribute('href'))
      .toBe('/admin-new/sessions/session-1/recordings');
    expect(byTestId(target, 'session-subarea-recordings').getAttribute('aria-current')).toBe('page');
  });

  it('links the network area after the full route set is enabled', () => {
    const target = renderComponent(SessionSubareaNavigation, {
      sessionId: 'session-1',
      activeId: 'network',
      availableIds: ['overview', 'live', 'files', 'recordings', 'network'],
    });

    expect(byTestId(target, 'session-subarea-network').getAttribute('href'))
      .toBe('/admin-new/sessions/session-1/network');
    expect(byTestId(target, 'session-subarea-network').getAttribute('aria-current')).toBe('page');
  });
});
