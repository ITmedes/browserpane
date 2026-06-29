import { describe, expect, it } from 'vitest';

import { sessionResource } from '$lib/test-utils/session-fixtures';
import { buildSessionOverviewModel } from './session-overview-view-model';

describe('buildSessionOverviewModel', () => {
  it('summarizes session capacity and attention states', () => {
    const model = buildSessionOverviewModel([
      sessionResource({ id: 'active-session', totalClients: 1 }),
      sessionResource({ id: 'queued-session', state: 'queued', queued: true, totalClients: 0 }),
      sessionResource({
        id: 'blocked-egress',
        totalClients: 0,
        egressHealth: 'blocked',
        admissionState: 'rejected',
        admissionReasonCode: 'project_paused',
      }),
    ]);

    expect(model.metrics.map((metric) => [metric.label, metric.value])).toEqual([
      ['Sessions', '3'],
      ['Active', '3'],
      ['Queued', '1'],
      ['Live clients', '1'],
    ]);
    expect(model.rows.find((row) => row.id === 'queued-session')?.attention).toBe('queued #2');
    expect(model.rows.find((row) => row.id === 'blocked-egress')?.attention).toBe('project_paused');
  });
});
