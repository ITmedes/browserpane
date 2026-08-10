import { afterEach, describe, expect, it } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import { projectResourceFixture } from '$lib/test-utils/project-fixture';
import ProjectUsageEvidence from './ProjectUsageEvidence.svelte';

afterEach(cleanupRenderedComponents);

describe('ProjectUsageEvidence', () => {
  it('renders capacity, enforcement, alerts, and sanitized egress counters', () => {
    const target = renderComponent(ProjectUsageEvidence, {
      project: projectResourceFixture({
        policy: { usage_budget_enforcement: 'block_session_creation' },
        usage: {
          active_sessions: 4,
          max_active_sessions: 4,
          egress_rx_bytes: 1_024,
          egress_tx_bytes: 2_048,
          egress_total_bytes: 3_072,
          alerts: [{
            metric: 'session_creations',
            state: 'approaching_limit',
            current_value: 8,
            limit_value: 10,
            threshold_percent: 100,
            message: 'Session creation budget is approaching its limit.',
          }],
        },
      }),
    });

    expect(byTestId(target, 'project-budget-enforcement').textContent).toContain('Block new sessions');
    expect(byTestId(target, 'project-usage-active_sessions').textContent).toContain('At limit');
    expect(byTestId(target, 'project-egress-breakdown').textContent).toContain('RX 1.0 KB / TX 2.0 KB');
    expect(byTestId(target, 'project-governance-alerts').textContent).toContain(
      'Session creation budget is approaching its limit',
    );
  });
});
