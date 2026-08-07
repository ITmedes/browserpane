import { formatDateTime, type ProjectTone } from '$lib/projects/project-formatters';
import type { ProjectResource } from '$lib/projects/project-types';
import { labelsToText, parseIdentityLabels, shortIdentityId, splitIdentityList } from './identity-form-utils';
import type {
  IdentityPrincipalResource,
  IdentityServicePrincipalReviewResource,
  ServicePrincipalState,
  UpsertServicePrincipalRequest,
} from './identity-types';

export type ServicePrincipalDraft = {
  readonly name: string;
  readonly description: string;
  readonly clientId: string;
  readonly issuer: string;
  readonly labelsText: string;
  readonly scopesText: string;
  readonly allowedProjectIds: readonly string[];
  readonly state: ServicePrincipalState;
};

export type ServicePrincipalFieldErrors = {
  readonly name: readonly string[];
  readonly clientId: readonly string[];
  readonly issuer: readonly string[];
  readonly labels: readonly string[];
};

export type ServicePrincipalValidation = {
  readonly valid: boolean;
  readonly request: UpsertServicePrincipalRequest | null;
  readonly fieldErrors: ServicePrincipalFieldErrors;
};

export type ServicePrincipalCatalogRow = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly clientId: string;
  readonly issuer: string;
  readonly state: ServicePrincipalState;
  readonly stateTone: ProjectTone;
  readonly scopes: string;
  readonly projects: string;
  readonly delegation: string;
  readonly lastActivity: string;
};

export function createServicePrincipalDraft(principal: IdentityPrincipalResource): ServicePrincipalDraft {
  return {
    name: '',
    description: '',
    clientId: '',
    issuer: principal.issuer,
    labelsText: '',
    scopesText: '',
    allowedProjectIds: [],
    state: 'active',
  };
}

export function editServicePrincipalDraft(resource: IdentityServicePrincipalReviewResource): ServicePrincipalDraft {
  return {
    name: resource.name,
    description: resource.description ?? '',
    clientId: resource.client_id,
    issuer: resource.issuer,
    labelsText: labelsToText(resource.labels),
    scopesText: resource.scopes.join('\n'),
    allowedProjectIds: [...resource.allowed_project_ids],
    state: resource.state,
  };
}

export function validateServicePrincipalDraft(draft: ServicePrincipalDraft): ServicePrincipalValidation {
  const fieldErrors = {
    name: draft.name.trim() ? [] : ['Name is required.'],
    clientId: draft.clientId.trim() ? [] : ['Client id is required.'],
    issuer: draft.issuer.trim() ? [] : ['Issuer is required.'],
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
      client_id: draft.clientId.trim(),
      issuer: draft.issuer.trim(),
      labels: labels.value,
      scopes: splitIdentityList(draft.scopesText),
      allowed_project_ids: [...new Set(draft.allowedProjectIds)],
      state: draft.state,
    },
  };
}

export function servicePrincipalStateRequest(
  resource: IdentityServicePrincipalReviewResource,
  state: ServicePrincipalState,
): UpsertServicePrincipalRequest {
  return {
    name: resource.name,
    description: resource.description,
    client_id: resource.client_id,
    issuer: resource.issuer,
    labels: resource.labels,
    scopes: resource.scopes,
    allowed_project_ids: resource.allowed_project_ids,
    state,
  };
}

export function buildServicePrincipalRows(
  resources: readonly IdentityServicePrincipalReviewResource[],
  projects: readonly ProjectResource[],
  search = '',
): readonly ServicePrincipalCatalogRow[] {
  const projectNames = new Map(projects.map((project) => [project.id, project.name]));
  const needle = search.trim().toLowerCase();
  return resources
    .map((resource) => ({
      id: resource.id,
      name: resource.name,
      description: resource.description ?? 'No description',
      clientId: resource.client_id,
      issuer: resource.issuer,
      state: resource.state,
      stateTone: resource.state === 'active' ? 'success' as const : 'danger' as const,
      scopes: resource.scopes.length > 0 ? resource.scopes.join(', ') : 'No intended scopes',
      projects: resource.allowed_project_ids.length > 0
        ? resource.allowed_project_ids.map((id) => projectNames.get(id) ?? shortIdentityId(id)).join(', ')
        : 'All projects metadata unset',
      delegation: `${resource.active_delegated_session_count} / ${resource.delegated_session_count} active`,
      lastActivity: resource.last_delegated_at
        ? `Delegated ${formatDateTime(resource.last_delegated_at)}`
        : resource.last_seen_at
          ? `Seen ${formatDateTime(resource.last_seen_at)}`
          : 'Not observed',
    }))
    .filter((row) => !needle || [
      row.id,
      row.name,
      row.clientId,
      row.issuer,
      row.state,
      row.scopes,
      row.projects,
    ].some((value) => value.toLowerCase().includes(needle)));
}
