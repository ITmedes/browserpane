import { describe, expect, it } from 'vitest';

import { workflowRunFixture } from '$lib/test-utils/workflow-run-fixture';
import { buildSessionAutomationModel } from './session-automation-view-model';

describe('session automation view model', () => {
  it('filters exact session associations, sorts newest first, and summarizes states', () => {
    const model = buildSessionAutomationModel('session-1', [
      workflowRunFixture({
        id: 'run-old',
        state: 'succeeded',
        completed_at: '2026-08-07T10:04:00.000Z',
        updated_at: '2026-08-07T10:04:00.000Z',
        intervention: { pending_request: null, last_resolution: null },
      }),
      workflowRunFixture({
        id: 'run-other',
        session_id: 'session-10',
        updated_at: '2026-08-07T10:07:00.000Z',
      }),
      workflowRunFixture({
        id: 'run-new',
        state: 'awaiting_input',
        updated_at: '2026-08-07T10:06:00.000Z',
      }),
    ]);

    expect(model.metrics.map((metric) => [metric.label, metric.value])).toEqual([
      ['Associated runs', '2'],
      ['Active', '1'],
      ['Needs attention', '1'],
    ]);
    expect(model.workflowRuns.map((run) => run.id)).toEqual(['run-new', 'run-old']);
  });

  it('returns an empty association model when the session has no workflow runs', () => {
    const model = buildSessionAutomationModel('session-2', [workflowRunFixture()]);

    expect(model.metrics.map((metric) => metric.value)).toEqual(['0', '0', '0']);
    expect(model.workflowRuns).toEqual([]);
  });
});
