import { describe, expect, it } from 'vitest';

import type { ProjectPolicyOption } from '$lib/projects/project-types';
import { projectResourceFixture } from '$lib/test-utils/project-fixture';

import {
  createNewSessionCreateDraft,
  validateSessionCreateDraft,
  type SessionCreateOptions,
} from './session-create-view-model';

describe('session create project policy validation', () => {
  it('rejects resources excluded by the selected project allowlists', () => {
    const project = projectResourceFixture({
      policy: {
        allowed_session_template_ids: ['template-allowed'],
        allowed_browser_context_ids: ['context-allowed'],
        allowed_egress_profile_ids: ['egress-allowed'],
      },
    });
    const validation = validateSessionCreateDraft({
      ...createNewSessionCreateDraft(),
      projectId: project.id,
      templateId: 'template-blocked',
      browserContextMode: 'reusable',
      browserContextId: 'context-blocked',
      egressProfileId: 'egress-blocked',
    }, {
      projects: [project],
      sessionTemplates: [option('template-blocked')],
      browserContexts: [option('context-blocked')],
      egressProfiles: [option('egress-blocked')],
    });

    expect(validation.valid).toBe(false);
    expect(validation.request).toBeNull();
    expect(validation.fieldErrors.templateId?.[0]).toContain('not included');
    expect(validation.fieldErrors.browserContextId?.[0]).toContain('not included');
    expect(validation.fieldErrors.egressProfileId?.[0]).toContain('not included');
  });

  it('rejects project-bound and disabled resources for owner-scoped sessions', () => {
    const options: SessionCreateOptions = {
      projects: [],
      sessionTemplates: [option('template-disabled', null, 'disabled')],
      browserContexts: [],
      egressProfiles: [option('egress-project', 'project-1')],
    };
    const disabledTemplate = validateSessionCreateDraft({
      ...createNewSessionCreateDraft(),
      templateId: 'template-disabled',
    }, options);
    const projectEgress = validateSessionCreateDraft({
      ...createNewSessionCreateDraft(),
      egressProfileId: 'egress-project',
    }, options);

    expect(disabledTemplate.fieldErrors.templateId?.[0]).toContain('disabled');
    expect(projectEgress.fieldErrors.egressProfileId?.[0]).toContain('requires project project-1');
  });
});

function option(
  id: string,
  projectId: string | null = null,
  state = 'ready',
): ProjectPolicyOption {
  return {
    id,
    projectId,
    name: id,
    description: null,
    state,
    scope: projectId ? `Project ${projectId}` : 'Owner scoped',
  };
}
