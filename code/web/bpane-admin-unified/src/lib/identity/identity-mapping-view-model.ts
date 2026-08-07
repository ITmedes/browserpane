import { formatDateTime, type ProjectTone } from '$lib/projects/project-formatters';
import type { ProjectResource } from '$lib/projects/project-types';
import { labelsToText, parseIdentityLabels, shortIdentityId, splitIdentityList } from './identity-form-utils';
import { mappingKindLabel } from './identity-review-view-model';
import type {
  IdentityMappingKind,
  IdentityMappingReviewResource,
  IdentityMappingState,
  IdentityPrincipalResource,
  IdentityServicePrincipalReviewResource,
  UpsertIdentityMappingRequest,
} from './identity-types';

export type IdentityMappingDraft = {
  readonly name: string;
  readonly description: string;
  readonly kind: IdentityMappingKind;
  readonly issuer: string;
  readonly externalId: string;
  readonly claimName: string;
  readonly servicePrincipalId: string;
  readonly projectId: string;
  readonly labelsText: string;
  readonly scopesText: string;
  readonly state: IdentityMappingState;
};

export type IdentityMappingFieldErrors = {
  readonly name: readonly string[];
  readonly issuer: readonly string[];
  readonly externalId: readonly string[];
  readonly claimName: readonly string[];
  readonly servicePrincipalId: readonly string[];
  readonly projectId: readonly string[];
  readonly labels: readonly string[];
};

export type IdentityMappingValidation = {
  readonly valid: boolean;
  readonly request: UpsertIdentityMappingRequest | null;
  readonly fieldErrors: IdentityMappingFieldErrors;
};

export type IdentityMappingCatalogRow = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly kind: string;
  readonly externalIdentity: string;
  readonly issuer: string;
  readonly project: string;
  readonly state: IdentityMappingState;
  readonly stateTone: ProjectTone;
  readonly effectiveness: string;
  readonly effectivenessTone: ProjectTone;
  readonly scopes: string;
  readonly updatedAt: string;
};

export function createIdentityMappingDraft(
  principal: IdentityPrincipalResource,
  projects: readonly ProjectResource[],
): IdentityMappingDraft {
  return {
    name: principal.display_name ? `${principal.display_name} project access` : '',
    description: '',
    kind: 'user',
    issuer: principal.issuer,
    externalId: principal.subject,
    claimName: '',
    servicePrincipalId: '',
    projectId: projects.find((project) => project.state === 'active')?.id ?? projects[0]?.id ?? '',
    labelsText: '',
    scopesText: '',
    state: 'active',
  };
}

export function editIdentityMappingDraft(resource: IdentityMappingReviewResource): IdentityMappingDraft {
  return {
    name: resource.name,
    description: resource.description ?? '',
    kind: resource.kind,
    issuer: resource.issuer,
    externalId: resource.external_id,
    claimName: resource.claim_name ?? '',
    servicePrincipalId: resource.service_principal_id ?? '',
    projectId: resource.project_id,
    labelsText: labelsToText(resource.labels),
    scopesText: resource.scopes.join('\n'),
    state: resource.state,
  };
}

export function selectMappingServicePrincipal(
  draft: IdentityMappingDraft,
  servicePrincipal: IdentityServicePrincipalReviewResource | null,
): IdentityMappingDraft {
  if (!servicePrincipal) {
    return { ...draft, servicePrincipalId: '', issuer: '', externalId: '' };
  }
  return {
    ...draft,
    kind: 'service_principal',
    servicePrincipalId: servicePrincipal.id,
    issuer: servicePrincipal.issuer,
    externalId: servicePrincipal.client_id,
    name: draft.name.trim() || `${servicePrincipal.name} project access`,
  };
}

export function validateIdentityMappingDraft(draft: IdentityMappingDraft): IdentityMappingValidation {
  const fieldErrors = {
    name: draft.name.trim() ? [] : ['Name is required.'],
    issuer: draft.issuer.trim() ? [] : ['Issuer is required.'],
    externalId: draft.externalId.trim() ? [] : ['External identity is required.'],
    claimName: draft.kind !== 'claim' || draft.claimName.trim() ? [] : ['Claim name is required.'],
    servicePrincipalId: draft.kind !== 'service_principal' || draft.servicePrincipalId.trim()
      ? []
      : ['Registered service principal is required.'],
    projectId: draft.projectId.trim() ? [] : ['Project is required.'],
    labels: [] as string[],
  };
  const labels = parseIdentityLabels(draft.labelsText);
  if (!labels.ok) {
    fieldErrors.labels = [labels.error];
  }
  if (Object.values(fieldErrors).some((errors) => errors.length > 0) || !labels.ok) {
    return { valid: false, request: null, fieldErrors };
  }
  return {
    valid: true,
    fieldErrors,
    request: {
      name: draft.name.trim(),
      description: draft.description.trim() || null,
      kind: draft.kind,
      issuer: draft.issuer.trim(),
      external_id: draft.externalId.trim(),
      claim_name: draft.kind === 'claim' ? draft.claimName.trim() : null,
      service_principal_id: draft.kind === 'service_principal' ? draft.servicePrincipalId.trim() : null,
      project_id: draft.projectId.trim(),
      labels: labels.value,
      scopes: splitIdentityList(draft.scopesText),
      state: draft.state,
    },
  };
}

export function identityMappingStateRequest(
  resource: IdentityMappingReviewResource,
  state: IdentityMappingState,
): UpsertIdentityMappingRequest {
  return {
    name: resource.name,
    description: resource.description,
    kind: resource.kind,
    issuer: resource.issuer,
    external_id: resource.external_id,
    claim_name: resource.claim_name,
    service_principal_id: resource.service_principal_id,
    project_id: resource.project_id,
    labels: resource.labels,
    scopes: resource.scopes,
    state,
  };
}

export function buildIdentityMappingRows(
  resources: readonly IdentityMappingReviewResource[],
  projects: readonly ProjectResource[],
  search = '',
): readonly IdentityMappingCatalogRow[] {
  const projectNames = new Map(projects.map((project) => [project.id, project.name]));
  const needle = search.trim().toLowerCase();
  return resources
    .map((resource) => ({
      id: resource.id,
      name: resource.name,
      description: resource.description ?? 'No description',
      kind: mappingKindLabel(resource.kind),
      externalIdentity: resource.claim_name
        ? `${resource.claim_name}=${resource.external_id}`
        : resource.external_id,
      issuer: resource.issuer,
      project: projectNames.get(resource.project_id) ?? shortIdentityId(resource.project_id),
      state: resource.state,
      stateTone: resource.state === 'active' ? 'success' as const : 'danger' as const,
      effectiveness: resource.effective_for_principal ? 'effective' : 'not effective',
      effectivenessTone: resource.effective_for_principal ? 'success' as const : 'neutral' as const,
      scopes: resource.scopes.length > 0 ? resource.scopes.join(', ') : 'No intended scopes',
      updatedAt: formatDateTime(resource.updated_at),
    }))
    .filter((row) => !needle || [
      row.id,
      row.name,
      row.kind,
      row.externalIdentity,
      row.issuer,
      row.project,
      row.state,
      row.effectiveness,
      row.scopes,
    ].some((value) => value.toLowerCase().includes(needle)));
}
