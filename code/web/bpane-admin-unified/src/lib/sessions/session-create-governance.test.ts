import { describe, expect, it } from 'vitest';

import { projectResourceFixture } from '$lib/test-utils/project-fixture';

import { SessionCreateGovernancePresenter } from './session-create-governance';
import type { SessionCreateOptions } from './session-create-view-model';

describe('SessionCreateGovernancePresenter', () => {
  const presenter = new SessionCreateGovernancePresenter();

  it('shows selected-project pressure and resource decisions', () => {
    const model = presenter.build('project-1', options());

    expect(model.project?.name).toBe('Support');
    expect(model.projectUsage?.metrics.find((metric) => metric.id === 'active_sessions')).toMatchObject({
      state: 'at_limit',
      displayValue: '1 / 1',
    });
    expect(model.sessionTemplates).toEqual(expect.arrayContaining([
      expect.objectContaining({ id: 'template-allowed', disabled: false }),
      expect.objectContaining({ id: 'template-blocked', disabled: true }),
    ]));
  });

  it('blocks project-scoped resources when no project is selected', () => {
    const model = presenter.build('', options());

    expect(model.egressProfiles).toEqual([
      expect.objectContaining({
        id: 'egress-project',
        disabled: true,
        decision: expect.objectContaining({ code: 'blocked_project_scope' }),
      }),
    ]);
  });
});

function options(): SessionCreateOptions {
  return {
    projects: [projectResourceFixture({
      quotas: { max_active_sessions: 1 },
      policy: { allowed_session_template_ids: ['template-allowed'] },
      usage: { active_sessions: 1, max_active_sessions: 1 },
    })],
    sessionTemplates: [
      option('template-allowed'),
      option('template-blocked'),
    ],
    browserContexts: [],
    egressProfiles: [option('egress-project', 'project-1')],
  };
}

function option(id: string, projectId: string | null = null) {
  return {
    id,
    projectId,
    name: id,
    description: null,
    state: 'ready',
    scope: projectId ? `Project ${projectId}` : 'Owner scoped',
  };
}
