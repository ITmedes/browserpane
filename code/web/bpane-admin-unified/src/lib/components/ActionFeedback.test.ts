import { afterEach, describe, expect, it } from 'vitest';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ActionFeedback from './ActionFeedback.svelte';

afterEach(cleanupRenderedComponents);

describe('ActionFeedback', () => {
  it('reserves a stable slot while idle without announcing a message', () => {
    const target = render({ status: 'idle' });

    expect(byTestId(target, 'action-feedback-slot').classList.contains('min-h-14')).toBe(true);
    expect(target.querySelector('[role="status"], [role="alert"]')).toBeNull();
  });

  it('renders running and success states with the supplied selectors', () => {
    const running = render({ status: 'running', label: 'Saving...' });
    expect(byTestId(running, 'resource-running').textContent).toContain('Saving...');
    expect(byTestId(running, 'resource-running').getAttribute('aria-live')).toBe('polite');

    const success = render({ status: 'success', message: 'Saved.' });
    expect(byTestId(success, 'resource-success').textContent).toContain('Saved.');
  });

  it('announces errors assertively and can omit reserved space', () => {
    const target = renderComponent(ActionFeedback, {
      ...baseProps,
      state: { status: 'error', message: 'Conflict.' },
      reserveSpace: false,
    });

    expect(byTestId(target, 'resource-error').getAttribute('role')).toBe('alert');
    expect(byTestId(target, 'action-feedback-slot').className).toBe('');
  });
});

const baseProps = {
  successTitle: 'Action completed',
  errorTitle: 'Action failed',
  successTestId: 'resource-success',
  errorTestId: 'resource-error',
  runningTestId: 'resource-running',
};

function render(state: Record<string, unknown>): HTMLElement {
  return renderComponent(ActionFeedback, { ...baseProps, state });
}
