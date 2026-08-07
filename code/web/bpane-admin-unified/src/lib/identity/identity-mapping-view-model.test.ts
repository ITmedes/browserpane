import { describe, expect, it } from 'vitest';

import { identityAccessReviewFixture } from '$lib/test-utils/identity-fixtures';
import {
  buildIdentityMappingRows,
  createIdentityMappingDraft,
  editIdentityMappingDraft,
  identityMappingStateRequest,
  selectMappingServicePrincipal,
  validateIdentityMappingDraft,
} from './identity-mapping-view-model';

describe('identity mapping view model', () => {
  const review = identityAccessReviewFixture();
  const resource = review.identity_mappings[0]!;

  it('defaults a new mapping to the current principal and first active project', () => {
    expect(createIdentityMappingDraft(review.principal, review.projects)).toMatchObject({
      name: 'Demo Operator project access',
      kind: 'user',
      issuer: review.principal.issuer,
      externalId: review.principal.subject,
      projectId: 'project-1',
    });
  });

  it('validates conditional mapping fields and labels next to their controls', () => {
    const base = createIdentityMappingDraft(review.principal, review.projects);
    const claim = validateIdentityMappingDraft({ ...base, kind: 'claim', claimName: '' });
    const servicePrincipal = validateIdentityMappingDraft({
      ...base,
      kind: 'service_principal',
      servicePrincipalId: '',
    });
    const labels = validateIdentityMappingDraft({ ...base, labelsText: 'broken' });

    expect(claim.fieldErrors.claimName).toEqual(['Claim name is required.']);
    expect(servicePrincipal.fieldErrors.servicePrincipalId).toEqual([
      'Registered service principal is required.',
    ]);
    expect(labels.fieldErrors.labels).toEqual(['Labels must use key=value format.']);
  });

  it('selects registered principals and builds kind-safe requests', () => {
    const selected = selectMappingServicePrincipal(
      { ...createIdentityMappingDraft(review.principal, review.projects), name: '' },
      review.service_principals[0]!,
    );
    const validation = validateIdentityMappingDraft(selected);

    expect(selected).toMatchObject({
      name: 'MCP bridge project access',
      servicePrincipalId: 'principal-1',
      externalId: 'bpane-mcp-bridge',
    });
    expect(validation.request).toMatchObject({
      kind: 'service_principal',
      service_principal_id: 'principal-1',
      claim_name: null,
    });
    expect(identityMappingStateRequest(resource, 'disabled').state).toBe('disabled');
  });

  it('restores edit drafts and resolves catalog project/effectiveness evidence', () => {
    expect(editIdentityMappingDraft(resource)).toMatchObject({
      name: 'Demo operator access',
      projectId: 'project-1',
      labelsText: 'source=admin',
    });
    expect(buildIdentityMappingRows(review.identity_mappings, review.projects, 'operations')[0]).toMatchObject({
      project: 'Operations',
      effectiveness: 'effective',
      effectivenessTone: 'success',
    });
    expect(buildIdentityMappingRows(review.identity_mappings, review.projects, 'missing')).toEqual([]);
  });
});
