import { afterEach, describe, expect, it } from 'vitest';

import type { BrowserContextStatusSummaryModel } from '$lib/browser-contexts/browser-context-edit-view-model';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import BrowserContextStatusSummary from './BrowserContextStatusSummary.svelte';

afterEach(cleanupRenderedComponents);

describe('BrowserContextStatusSummary', () => {
  it('renders state, persistence, and status items', () => {
    const target = renderComponent(BrowserContextStatusSummary, {
      model: summary(),
    });

    expect(byTestId(target, 'browser-context-status-summary').textContent).toContain('context-1');
    expect(byTestId(target, 'browser-context-status-summary').textContent).toContain('ready');
    expect(byTestId(target, 'browser-context-status-summary').textContent).toContain('reusable');
    expect(byTestId(target, 'browser-context-status-summary').textContent).toContain('Storage');
  });
});

function summary(): BrowserContextStatusSummaryModel {
  return {
    contextId: 'context-1',
    stateLabel: 'ready',
    stateTone: 'success',
    persistenceLabel: 'reusable',
    persistenceTone: 'success',
    items: [
      { label: 'Scope', value: 'Owner scoped', tone: 'neutral' },
      { label: 'Storage', value: '1 MB / 1 GB', tone: 'success' },
    ],
  };
}
