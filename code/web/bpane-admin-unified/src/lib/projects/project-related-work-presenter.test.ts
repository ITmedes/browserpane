import { describe, expect, it } from 'vitest';

import { sessionResource } from '$lib/test-utils/session-fixtures';
import { workflowRunFixture } from '$lib/test-utils/workflow-run-fixture';

import { ProjectRelatedWorkPresenter } from './project-related-work-presenter';

describe('ProjectRelatedWorkPresenter', () => {
  const presenter = new ProjectRelatedWorkPresenter();

  it('filters resources by project and retains structured session queue evidence', () => {
    const queued = sessionResource({ id: 'session/queued', queued: true });
    const unrelated = {
      ...sessionResource({ id: 'other-session' }),
      project_id: 'project-2',
    };

    const model = presenter.build('project-1', [unrelated, queued], []);

    expect(model.sessions).toHaveLength(1);
    expect(model.queuedSessions).toBe(1);
    expect(model.sessions[0]).toMatchObject({
      id: 'session/queued',
      href: '/admin-new/sessions/session%2Fqueued',
      admissionState: 'allowed',
      reasonCode: 'policy_ok',
      queuePosition: 2,
      tone: 'warning',
    });
  });

  it('shows project denial and generic workflow queue evidence without inventing position', () => {
    const denied = workflowRunFixture({
      id: 'run/denied',
      state: 'failed',
      project_admission: {
        state: 'denied',
        reason_code: 'project_inactive',
        message: 'Project is inactive.',
        checked_at: '2026-08-10T09:00:00.000Z',
      },
      admission: null,
      updated_at: '2026-08-10T09:00:00.000Z',
    });
    const queued = workflowRunFixture({
      id: 'run-queued',
      state: 'queued',
      project_admission: null,
      admission: {
        state: 'queued',
        reason: 'runtime_capacity',
        queued_at: '2026-08-10T09:01:00.000Z',
      },
      updated_at: '2026-08-10T09:01:00.000Z',
    });

    const model = presenter.build('project-1', [], [denied, queued]);

    expect(model.queuedWorkflowRuns).toBe(1);
    expect(model.workflowRuns[0]).toMatchObject({
      id: 'run-queued',
      reasonCode: 'runtime_capacity',
      queuePosition: null,
      tone: 'warning',
    });
    expect(model.workflowRuns[1]).toMatchObject({
      id: 'run/denied',
      href: '/admin-new/workflow-runs/run%2Fdenied',
      admissionState: 'denied',
      reasonCode: 'project_inactive',
      message: 'Project is inactive.',
      tone: 'danger',
    });
  });
});
