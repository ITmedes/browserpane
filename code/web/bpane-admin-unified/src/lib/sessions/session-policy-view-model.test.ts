import { describe, expect, it } from 'vitest';

import type { ProjectResource } from '$lib/projects/project-types';
import { sessionResource, sessionStatus } from '$lib/test-utils/session-fixtures';
import { buildSessionPolicyModel } from './session-policy-view-model';

describe('session policy view model', () => {
  it('keeps project restrictions distinct from effective session capabilities', () => {
    const session = {
      ...sessionResource(),
      capabilities: {
        ...sessionResource().capabilities,
        file_transfer: false,
      },
    };
    const model = buildSessionPolicyModel(session, sessionStatus(), projectFixture());

    expect(findFact(model, 'session-policy-capability-file_transfer')).toMatchObject({
      value: 'Disabled',
      tone: 'warning',
      description: 'Disabled because the project blocks browser uploads.',
    });
    expect(findFact(model, 'session-policy-browser-upload')).toMatchObject({
      value: 'Blocked',
      tone: 'warning',
    });
    expect(findFact(model, 'session-policy-browser-download')).toMatchObject({
      value: 'Allowed',
      tone: 'success',
    });
    expect(findFact(model, 'session-policy-template')).toMatchObject({
      value: 'template-1',
      tone: 'success',
    });
    expect(findFact(model, 'session-policy-egress-profile')).toMatchObject({
      value: 'egress-1',
      tone: 'danger',
    });
  });

  it('describes owner-scoped sessions without inventing project restrictions', () => {
    const model = buildSessionPolicyModel(
      sessionResource({ projectName: null }),
      null,
      null,
    );

    expect(model.scopeLabel).toBe('Owner-scoped defaults');
    expect(findFact(model, 'session-policy-browser-upload')).toMatchObject({
      value: 'Owner scope',
      tone: 'neutral',
    });
    expect(findFact(model, 'session-policy-admission-decision').value).toBe('Owner scope');
    expect(findFact(model, 'session-policy-stop-eligibility').value).toBe('Not loaded');
  });

  it('labels docker browser policy as startup evidence rather than an active probe', () => {
    const model = buildSessionPolicyModel(sessionResource(), sessionStatus(), projectFixture());

    expect(findFact(model, 'session-policy-local-file-mode')).toMatchObject({
      value: 'Startup-enforced deny-all',
      tone: 'success',
    });
    expect(model.sections.find((section) => section.testId === 'session-policy-browser-runtime')?.description)
      .toContain('not an active browser probe');
  });
});

function findFact(model: ReturnType<typeof buildSessionPolicyModel>, testId: string) {
  const fact = model.sections.flatMap((section) => section.facts).find((candidate) => candidate.testId === testId);
  expect(fact).toBeDefined();
  return fact!;
}

function projectFixture(): ProjectResource {
  return {
    id: 'project-1',
    name: 'Support',
    description: 'Support browser work',
    labels: {},
    quotas: {},
    policy: {
      allowed_session_template_ids: ['template-1'],
      allowed_egress_profile_ids: ['egress-2'],
      allowed_extension_ids: [],
      allowed_browser_context_ids: ['context-1'],
      allowed_file_workspace_ids: ['workspace-1'],
      allow_browser_uploads: false,
      allow_browser_downloads: true,
      allow_session_file_bindings: false,
      allow_manual_recordings: false,
      usage_budget_enforcement: 'block_session_creation',
    },
    state: 'active',
    usage: {
      project_id: 'project-1',
      active_sessions: 1,
      queued_sessions: 0,
      session_creations: 1,
      active_workflow_runs: 0,
      runtime_usage_ms: 30_000,
      egress_rx_bytes: 0,
      egress_tx_bytes: 0,
      egress_total_bytes: 0,
      retained_storage_bytes: 0,
      alerts: [],
      observed_at: '2026-08-07T10:00:00Z',
    },
    created_at: '2026-08-07T09:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
  };
}
