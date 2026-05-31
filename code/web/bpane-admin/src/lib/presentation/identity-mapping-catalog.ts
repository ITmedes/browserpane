import type {
  CreateIdentityMappingCommand,
  IdentityMappingKind,
  IdentityMappingResource,
  IdentityMappingState,
  IdentityPrincipalResource,
  IdentityServicePrincipalReviewResource,
  ProjectResource,
} from '../api/control-types';

export type IdentityMappingFormInput = {
  readonly name: string;
  readonly description: string;
  readonly kind: IdentityMappingKind;
  readonly issuer: string;
  readonly externalId: string;
  readonly claimName: string;
  readonly servicePrincipalId: string;
  readonly projectId: string;
  readonly labels: string;
  readonly scopes: string;
  readonly state: IdentityMappingState;
};

export type IdentityMappingCommandResult =
  | { readonly ok: true; readonly command: CreateIdentityMappingCommand }
  | { readonly ok: false; readonly error: string };

export type IdentityMappingCatalogRow = {
  readonly id: string;
  readonly name: string;
  readonly kind: string;
  readonly externalIdentity: string;
  readonly projectId: string;
  readonly state: IdentityMappingState;
  readonly effective: string;
  readonly scopes: string;
  readonly updatedAt: string;
};

export function identityMappingRows(
  mappings: readonly (IdentityMappingResource & { readonly effective_for_principal?: boolean })[],
  search: string,
  projects: readonly ProjectResource[] = [],
): readonly IdentityMappingCatalogRow[] {
  const needle = search.trim().toLowerCase();
  const projectNames = new Map(projects.map((project) => [project.id, project.name]));
  return mappings
    .map((mapping) => ({
      id: mapping.id,
      name: mapping.name,
      kind: identityMappingKindLabel(mapping.kind),
      externalIdentity: mapping.claim_name ? `${mapping.claim_name}=${mapping.external_id}` : mapping.external_id,
      projectId: projectLabel(mapping.project_id, projectNames),
      state: mapping.state,
      effective: mapping.effective_for_principal ? 'effective' : 'not effective',
      scopes: mapping.scopes.length > 0 ? mapping.scopes.join(', ') : 'no scopes',
      updatedAt: mapping.updated_at,
    }))
    .filter((row) => {
      if (!needle) {
        return true;
      }
      return [
        row.id,
        row.name,
        row.kind,
        row.externalIdentity,
        row.projectId,
        row.state,
        row.effective,
        row.scopes,
      ].some((value) => value.toLowerCase().includes(needle));
    });
}

export function emptyIdentityMappingForm(
  principal: IdentityPrincipalResource | null,
  projects: readonly ProjectResource[],
): IdentityMappingFormInput {
  return {
    name: principal?.display_name ? `${principal.display_name} project access` : 'Project access mapping',
    description: '',
    kind: 'user',
    issuer: principal?.issuer ?? '',
    externalId: principal?.subject ?? '',
    claimName: '',
    servicePrincipalId: '',
    projectId: projects.find((project) => project.state === 'active')?.id ?? projects[0]?.id ?? '',
    labels: '',
    scopes: '',
    state: 'active',
  };
}

export function formFromIdentityMapping(mapping: IdentityMappingResource): IdentityMappingFormInput {
  return {
    name: mapping.name,
    description: mapping.description ?? '',
    kind: mapping.kind,
    issuer: mapping.issuer,
    externalId: mapping.external_id,
    claimName: mapping.claim_name ?? '',
    servicePrincipalId: mapping.service_principal_id ?? '',
    projectId: mapping.project_id,
    labels: Object.entries(mapping.labels)
      .map(([key, value]) => `${key}=${value}`)
      .join('\n'),
    scopes: mapping.scopes.join('\n'),
    state: mapping.state,
  };
}

export function formWithServicePrincipal(
  input: IdentityMappingFormInput,
  servicePrincipal: IdentityServicePrincipalReviewResource | null,
): IdentityMappingFormInput {
  if (!servicePrincipal) {
    return {
      ...input,
      servicePrincipalId: '',
      issuer: '',
      externalId: '',
    };
  }
  return {
    ...input,
    kind: 'service_principal',
    servicePrincipalId: servicePrincipal.id,
    issuer: servicePrincipal.issuer,
    externalId: servicePrincipal.client_id,
    name: input.name.trim() ? input.name : `${servicePrincipal.name} project access`,
  };
}

export function buildIdentityMappingCommand(input: IdentityMappingFormInput): IdentityMappingCommandResult {
  const name = input.name.trim();
  if (!name) {
    return { ok: false, error: 'Mapping name is required.' };
  }
  const issuer = input.issuer.trim();
  if (!issuer) {
    return { ok: false, error: 'Issuer is required.' };
  }
  const externalId = input.externalId.trim();
  if (!externalId) {
    return { ok: false, error: 'External identity is required.' };
  }
  const projectId = input.projectId.trim();
  if (!projectId) {
    return { ok: false, error: 'Project is required.' };
  }

  const labels = parseLabels(input.labels);
  if (!labels.ok) {
    return labels;
  }

  const claimName = input.claimName.trim();
  const servicePrincipalId = input.servicePrincipalId.trim();
  if (input.kind === 'claim' && !claimName) {
    return { ok: false, error: 'Claim mappings require a claim name.' };
  }
  if (input.kind === 'service_principal' && !servicePrincipalId) {
    return { ok: false, error: 'Service-principal mappings require a registered service principal.' };
  }

  const description = input.description.trim();
  const command: CreateIdentityMappingCommand = {
    name,
    ...(description ? { description } : {}),
    kind: input.kind,
    issuer,
    external_id: externalId,
    ...(input.kind === 'claim' ? { claim_name: claimName } : {}),
    ...(input.kind === 'service_principal' ? { service_principal_id: servicePrincipalId } : {}),
    project_id: projectId,
    labels: labels.value,
    scopes: splitList(input.scopes),
    state: input.state,
  };
  return { ok: true, command };
}

export function commandFromIdentityMapping(
  mapping: IdentityMappingResource,
  state: IdentityMappingState = mapping.state,
): CreateIdentityMappingCommand {
  return {
    name: mapping.name,
    ...(mapping.description ? { description: mapping.description } : {}),
    kind: mapping.kind,
    issuer: mapping.issuer,
    external_id: mapping.external_id,
    ...(mapping.claim_name ? { claim_name: mapping.claim_name } : {}),
    ...(mapping.service_principal_id ? { service_principal_id: mapping.service_principal_id } : {}),
    project_id: mapping.project_id,
    labels: mapping.labels,
    scopes: mapping.scopes,
    state,
  };
}

export function identityMappingKindLabel(kind: IdentityMappingKind | string): string {
  switch (kind) {
    case 'service_principal':
      return 'Service principal';
    case 'group':
      return 'Group';
    case 'claim':
      return 'Claim';
    case 'user':
    default:
      return 'User';
  }
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
