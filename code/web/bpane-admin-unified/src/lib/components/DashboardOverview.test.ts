import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DashboardSnapshot } from '$lib/dashboard/dashboard-overview-view-model';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import DashboardOverview from './DashboardOverview.svelte';

afterEach(cleanupRenderedComponents);

describe('DashboardOverview', () => {
  it('renders metrics, shortcuts, and refresh action for ready state', () => {
    const onRefresh = vi.fn();
    const target = renderComponent(DashboardOverview, {
      state: {
        status: 'ready',
        snapshot: emptySnapshot(),
        failures: [],
      },
      onRefresh,
    });

    expect(byTestId(target, 'dashboard-metric-sessions').textContent).toContain('Active sessions');
    expect(byTestId(target, 'dashboard-link-sessions').getAttribute('href')).toBe('/admin-new/sessions');
    expect(byTestId(target, 'dashboard-attention-empty').textContent).toContain('No catalog state');
    expect(byTestId(target, 'dashboard-activity-empty').textContent).toContain('No recent sessions');

    byTestId(target, 'dashboard-refresh').click();

    expect(onRefresh).toHaveBeenCalledOnce();
  });

  it('renders loading, error, and partial-load states', async () => {
    const loadingTarget = renderComponent(DashboardOverview, {
      state: { status: 'loading' },
    });
    expect(byTestId(loadingTarget, 'dashboard-loading').textContent).toContain('Loading dashboard');
    await cleanupRenderedComponents();

    const errorTarget = renderComponent(DashboardOverview, {
      state: { status: 'error', message: 'catalogs unavailable' },
    });
    expect(byTestId(errorTarget, 'dashboard-error').textContent).toContain('catalogs unavailable');
    await cleanupRenderedComponents();

    const partialTarget = renderComponent(DashboardOverview, {
      state: {
        status: 'ready',
        snapshot: emptySnapshot(),
        failures: [{ resource: 'Egress profiles', message: 'HTTP 503', href: '/admin-new/egress' }],
      },
    });
    expect(byTestId(partialTarget, 'dashboard-partial-warning').textContent).toContain('Egress profiles');
    expect(byTestId(partialTarget, 'dashboard-attention-list').textContent).toContain('Egress profiles unavailable');
  });
});

function emptySnapshot(): DashboardSnapshot {
  return {
    sessions: [],
    projects: [],
    browserContexts: [],
    egressProfiles: [],
    fileWorkspaces: [],
    workflows: [],
    workflowRuns: [],
    recordings: [],
  };
}
