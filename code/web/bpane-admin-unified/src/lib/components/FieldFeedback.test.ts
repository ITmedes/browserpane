import { afterEach, describe, expect, it } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import FieldFeedback from './FieldFeedback.svelte';

afterEach(cleanupRenderedComponents);

describe('FieldFeedback', () => {
  it('renders a stable hint row without an error test target', () => {
    const target = renderComponent(FieldFeedback, {
      hint: 'One value per line.',
      testId: 'field-error',
    });

    expect(target.textContent).toContain('One value per line.');
    expect(target.querySelector('[data-testid="field-error"]')).toBeNull();
  });

  it('renders one or more validation errors in the reserved row', async () => {
    let target = renderComponent(FieldFeedback, {
      errors: ['Name is required.'],
      testId: 'field-error',
    });

    expect(byTestId(target, 'field-error').textContent).toContain('Name is required.');

    await cleanupRenderedComponents();
    target = renderComponent(FieldFeedback, {
      errors: ['First issue.', 'Second issue.'],
      testId: 'field-errors',
    });

    expect(byTestId(target, 'field-errors').textContent).toContain('First issue.');
    expect(byTestId(target, 'field-errors').textContent).toContain('Second issue.');
  });
});
