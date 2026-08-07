import { describe, expect, it } from 'vitest';

import { identityAccessReviewFixture } from '$lib/test-utils/identity-fixtures';
import { buildIdentityReviewViewModel, mappingKindLabel } from './identity-review-view-model';

describe('buildIdentityReviewViewModel', () => {
  it('projects principal, project, resource, delegation, and safe signal evidence', () => {
    const model = buildIdentityReviewViewModel(identityAccessReviewFixture());

    expect(model.principalName).toBe('Demo Operator');
    expect(model.principalType).toBe('User');
    expect(model.principalFacts).toContainEqual({
      label: 'Client id',
      value: 'interactive user',
    });
    expect(model.metrics).toContainEqual({
      id: 'sessions',
      label: 'Active sessions',
      value: '1 / 2',
    });
    expect(model.projects[0]).toMatchObject({
      name: 'Operations',
      sessions: '1 / 3',
      workflowRuns: '1 / 2',
      stateTone: 'success',
    });
    expect(model.delegations[0]).toMatchObject({
      name: 'MCP bridge',
      registration: 'registered',
      state: 'active',
      sessions: '1 / 2 active',
    });
    expect(model.unmappedSignals[0]).toMatchObject({
      kind: 'Group',
      identity: 'Support',
      reason: 'Safe group signal has no active project mapping',
    });
  });

  it('labels supported mapping kinds', () => {
    expect(mappingKindLabel('service_principal')).toBe('Service principal');
    expect(mappingKindLabel('claim')).toBe('Claim');
  });
});
