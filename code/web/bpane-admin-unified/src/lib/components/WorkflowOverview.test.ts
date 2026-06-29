import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkflowDefinitionResource } from '$lib/workflows/workflow-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowOverview from './WorkflowOverview.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowOverview', () => {
  it('renders loading, error, empty, and ready states', () => {
    let target = renderComponent(WorkflowOverview, { state: { status: 'loading' } });
    expect(byTestId(target, 'workflows-loading').textContent).toContain('Loading workflows');

    void cleanupRenderedComponents();
    target = renderComponent(WorkflowOverview, {
      state: { status: 'error', message: 'Catalog unavailable.' },
    });
    expect(byTestId(target, 'workflows-error').textContent).toContain('Catalog unavailable');

    void cleanupRenderedComponents();
    target = renderComponent(WorkflowOverview, {
      state: { status: 'ready', definitions: [], versions: {}, hiddenCount: 0, includeHidden: false },
    });
    expect(byTestId(target, 'workflows-empty').textContent).toContain('catalog is empty');

    void cleanupRenderedComponents();
    target = renderComponent(WorkflowOverview, {
      state: {
        status: 'ready',
        definitions: [workflow()],
        versions: {},
        hiddenCount: 1,
        includeHidden: false,
      },
    });
    expect(byTestId(target, 'workflows-metric-visible').textContent).toContain('1');
    expect(byTestId(target, 'workflows-hidden-note').textContent).toContain('1 internal');
    expect(byTestId(target, 'workflows-list').textContent).toContain('Customer audit');
  });

  it('delegates refresh requests', () => {
    const onRefresh = vi.fn();
    const target = renderComponent(WorkflowOverview, {
      state: {
        status: 'ready',
        definitions: [workflow()],
        versions: {},
        hiddenCount: 0,
        includeHidden: false,
      },
      onRefresh,
    });

    byTestId(target, 'workflows-refresh-button').click();

    expect(onRefresh).toHaveBeenCalledOnce();
  });
});

function workflow(): WorkflowDefinitionResource {
  return {
    id: 'workflow-1',
    name: 'Customer audit',
    description: null,
    labels: {},
    latest_version: null,
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}
