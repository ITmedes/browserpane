import type {
  CreateServicePrincipalCommand,
  IdentityPrincipalResource,
  IdentityServicePrincipalReviewResource,
  ProjectResource,
  ServicePrincipalState,
} from '../api/control-types';

export type ServicePrincipalFormInput = {
  readonly name: string;
  readonly description: string;
  readonly clientId: string;
  readonly issuer: string;
  readonly labels: string;
  readonly scopes: string;
  readonly allowedProjectIds: readonly string[];
  readonly state: ServicePrincipalState;
};

export type ServicePrincipalCommandResult =
  | { readonly ok: true; readonly command: CreateServicePrincipalCommand }
  | { readonly ok: false; readonly error: string };

export type ServicePrincipalCatalogRow = {
  readonly id: string;
  readonly name: string;
  readonly clientId: string;
  readonly issuer: string;
  readonly state: ServicePrincipalState;
  readonly scopes: string;
  readonly projects: string;
  readonly delegatedSummary: string;
  readonly lastActivity: string;
};

export function servicePrincipalRows(
  servicePrincipals: readonly IdentityServicePrincipalReviewResource[],
  search: string,
  projects: readonly ProjectResource[] = [],
): readonly ServicePrincipalCatalogRow[] {
  const needle = search.trim().toLowerCase();
  const projectNames = new Map(projects.map((project) => [project.id, project.name]));
  return servicePrincipals
    .map((servicePrincipal) => ({
      id: servicePrincipal.id,
      name: servicePrincipal.name,
      clientId: servicePrincipal.client_id,
      issuer: servicePrincipal.issuer,
      state: servicePrincipal.state,
      scopes: servicePrincipal.scopes.length > 0 ? servicePrincipal.scopes.join(', ') : 'no scopes',
      projects: servicePrincipal.allowed_project_ids.length > 0
        ? servicePrincipal.allowed_project_ids.map((projectId) => projectLabel(projectId, projectNames)).join(', ')
        : 'all projects metadata unset',
      delegatedSummary: `${servicePrincipal.active_delegated_session_count}/${servicePrincipal.delegated_session_count} active`,
      lastActivity: servicePrincipal.last_delegated_at
        ? `delegated ${servicePrincipal.last_delegated_at}`
        : servicePrincipal.last_seen_at
          ? `seen ${servicePrincipal.last_seen_at}`
          : 'not observed',
    }))
    .filter((row) => {
      if (!needle) {
        return true;
      }
      return [
        row.id,
        row.name,
        row.clientId,
        row.issuer,
        row.state,
        row.scopes,
        row.projects,
      ].some((value) => value.toLowerCase().includes(needle));
    });
}

export function emptyServicePrincipalForm(
  principal: IdentityPrincipalResource | null,
): ServicePrincipalFormInput {
  return {
    name: 'Service principal',
    description: '',
    clientId: '',
    issuer: principal?.issuer ?? '',
    labels: '',
    scopes: '',
    allowedProjectIds: [],
    state: 'active',
  };
}

export function formFromServicePrincipal(
  servicePrincipal: IdentityServicePrincipalReviewResource,
): ServicePrincipalFormInput {
  return {
    name: servicePrincipal.name,
    description: servicePrincipal.description ?? '',
    clientId: servicePrincipal.client_id,
    issuer: servicePrincipal.issuer,
    labels: Object.entries(servicePrincipal.labels)
      .map(([key, value]) => `${key}=${value}`)
      .join('\n'),
    scopes: servicePrincipal.scopes.join('\n'),
    allowedProjectIds: [...servicePrincipal.allowed_project_ids],
    state: servicePrincipal.state,
  };
}

export function buildServicePrincipalCommand(
  input: ServicePrincipalFormInput,
): ServicePrincipalCommandResult {
  const name = input.name.trim();
  if (!name) {
    return { ok: false, error: 'Service principal name is required.' };
  }
  const clientId = input.clientId.trim();
  if (!clientId) {
    return { ok: false, error: 'Client id is required.' };
  }
  const issuer = input.issuer.trim();
  if (!issuer) {
    return { ok: false, error: 'Issuer is required.' };
  }

  const labels = parseLabels(input.labels);
  if (!labels.ok) {
    return labels;
  }

  const description = input.description.trim();
  const command: CreateServicePrincipalCommand = {
    name,
    ...(description ? { description } : {}),
    client_id: clientId,
    issuer,
    labels: labels.value,
    scopes: splitList(input.scopes),
    allowed_project_ids: [...input.allowedProjectIds],
    state: input.state,
  };
  return { ok: true, command };
}

export function commandFromServicePrincipal(
  servicePrincipal: IdentityServicePrincipalReviewResource,
  state: ServicePrincipalState = servicePrincipal.state,
): CreateServicePrincipalCommand {
  return {
    name: servicePrincipal.name,
    ...(servicePrincipal.description ? { description: servicePrincipal.description } : {}),
    client_id: servicePrincipal.client_id,
    issuer: servicePrincipal.issuer,
    labels: servicePrincipal.labels,
    scopes: servicePrincipal.scopes,
    allowed_project_ids: servicePrincipal.allowed_project_ids,
    state,
  };
}

function parseLabels(value: string): { readonly ok: true; readonly value: Record<string, string> } | { readonly ok: false; readonly error: string } {
  const labels: Record<string, string> = {};
  for (const entry of splitList(value)) {
    const separator = entry.indexOf('=');
    if (separator <= 0) {
      return { ok: false, error: 'Labels must use key=value format.' };
    }
    const key = entry.slice(0, separator).trim();
    const labelValue = entry.slice(separator + 1).trim();
    if (!key || !labelValue) {
      return { ok: false, error: 'Labels must include a non-empty key and value.' };
    }
    labels[key] = labelValue;
  }
  return { ok: true, value: labels };
}

function splitList(value: string): string[] {
  return value
    .split(/[\n,]/)
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function shortId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}

function projectLabel(projectId: string, projectNames: ReadonlyMap<string, string>): string {
  const name = projectNames.get(projectId);
  return name ? `${name} (${shortId(projectId)})` : shortId(projectId);
}
