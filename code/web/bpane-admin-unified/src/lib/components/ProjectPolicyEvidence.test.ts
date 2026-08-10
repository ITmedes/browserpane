import { afterEach, describe, expect, it } from 'vitest';

import type { ProjectPolicyOptions } from '$lib/projects/project-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import { projectResourceFixture } from '$lib/test-utils/project-fixture';
import ProjectPolicyEvidence from './ProjectPolicyEvidence.svelte';

afterEach(cleanupRenderedComponents);

describe('ProjectPolicyEvidence', () => {
  it('renders operation gates and resolved allowlist resources', () => {
    const target = renderComponent(ProjectPolicyEvidence, {
      project: projectResourceFixture({
        policy: {
          allow_browser_uploads: false,
          allowed_egress_profile_ids: ['egress-1'],
        },
      }),
      policyOptionsState: { status: 'ready', options: policyOptions() },
    });

    expect(byTestId(target, 'project-operation-browser_uploads').textContent).toContain('Blocked');
    expect(byTestId(target, 'project-policy-evidence').textContent).toContain('Support proxy');
    expect(byTestId(target, 'project-policy-evidence').textContent).toContain('restricted');
    expect(byTestId(target, 'project-policy-evidence').textContent).toContain('unrestricted');
  });

  it('keeps operation evidence visible when selector resources fail', () => {
    const target = renderComponent(ProjectPolicyEvidence, {
      project: projectResourceFixture(),
      policyOptionsState: { status: 'error', message: 'catalog unavailable' },
    });

    expect(byTestId(target, 'project-operation-browser_uploads').textContent).toContain('Allowed');
    expect(byTestId(target, 'project-policy-evidence-error').textContent).toContain('catalog unavailable');
  });
});

function policyOptions(): ProjectPolicyOptions {
  return {
    sessionTemplates: [],
    browserContexts: [],
    egressProfiles: [{
      id: 'egress-1',
      projectId: null,
      name: 'Support proxy',
      description: 'Approved proxy',
      state: 'ready',
      scope: 'Owner scoped',
    }],
    extensions: [],
    fileWorkspaces: [],
  };
}
