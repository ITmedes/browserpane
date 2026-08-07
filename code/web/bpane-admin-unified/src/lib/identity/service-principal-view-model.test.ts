import { describe, expect, it } from 'vitest';

import { identityAccessReviewFixture } from '$lib/test-utils/identity-fixtures';
import {
  buildServicePrincipalRows,
  createServicePrincipalDraft,
  editServicePrincipalDraft,
  servicePrincipalStateRequest,
  validateServicePrincipalDraft,
} from './service-principal-view-model';

describe('service principal view model', () => {
  const review = identityAccessReviewFixture();
  const resource = review.service_principals[0]!;

  it('creates safe defaults and validates required and label fields', () => {
    const draft = createServicePrincipalDraft(review.principal);
    expect(draft.issuer).toBe(review.principal.issuer);

    const invalid = validateServicePrincipalDraft({ ...draft, labelsText: 'invalid' });
    expect(invalid.valid).toBe(false);
    expect(invalid.fieldErrors.name).toEqual(['Name is required.']);
    expect(invalid.fieldErrors.clientId).toEqual(['Client id is required.']);
    expect(invalid.fieldErrors.labels).toEqual(['Labels must use key=value format.']);
  });

  it('builds a normalized request and lifecycle update', () => {
    const draft = {
      ...editServicePrincipalDraft(resource),
      scopesText: 'session:delegate\nsession:delegate,session:read',
      allowedProjectIds: ['project-1', 'project-1'],
    };
    const validation = validateServicePrincipalDraft(draft);

    expect(validation.request).toMatchObject({
      name: 'MCP bridge',
      labels: { purpose: 'automation' },
      scopes: ['session:delegate', 'session:read'],
      allowed_project_ids: ['project-1'],
    });
    expect(servicePrincipalStateRequest(resource, 'disabled').state).toBe('disabled');
  });

  it('resolves project names and filters catalog rows', () => {
    expect(buildServicePrincipalRows(review.service_principals, review.projects, 'operations')[0]).toMatchObject({
      name: 'MCP bridge',
      projects: 'Operations',
      delegation: '1 / 2 active',
      stateTone: 'success',
    });
    expect(buildServicePrincipalRows(review.service_principals, review.projects, 'missing')).toEqual([]);
  });
});
