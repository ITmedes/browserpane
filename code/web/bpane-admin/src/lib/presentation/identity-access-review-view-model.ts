import type {
  IdentityAccessReviewResponse,
  IdentityPrincipalType,
  ProjectResource,
} from '../api/control-types';

export type IdentityMetricRow = {
  readonly key: string;
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type IdentityProjectRow = {
  readonly id: string;
  readonly name: string;
  readonly state: string;
  readonly activeSessions: string;
  readonly activeWorkflowRuns: string;
  readonly retainedStorage: string;
};

export type IdentityDelegationRow = {
  readonly clientId: string;
  readonly issuer: string;
  readonly displayName: string;
  readonly registration: string;
  readonly state: string;
  readonly sessionSummary: string;
  readonly sessionIds: string;
};

export type IdentityServicePrincipalRow = {
  readonly id: string;
  readonly name: string;
  readonly clientId: string;
  readonly issuer: string;
  readonly state: string;
  readonly scopes: string;
  readonly projects: string;
  readonly delegatedSummary: string;
  readonly delegatedSessionIds: string;
  readonly lastActivity: string;
};

export type IdentityAccessReviewViewModel = {
  readonly principalTitle: string;
  readonly principalSubtitle: string;
  readonly principalTypeLabel: string;
  readonly generatedAtLabel: string;
  readonly metrics: readonly IdentityMetricRow[];
  readonly projects: readonly IdentityProjectRow[];
  readonly servicePrincipals: readonly IdentityServicePrincipalRow[];
  readonly delegations: readonly IdentityDelegationRow[];
};

export class IdentityAccessReviewViewModelBuilder {
  static build(review: IdentityAccessReviewResponse): IdentityAccessReviewViewModel {
    return {
      principalTitle: review.principal.display_name ?? review.principal.subject,
      principalSubtitle: `${review.principal.issuer} / ${review.principal.subject}`,
      principalTypeLabel: principalTypeLabel(review.principal.principal_type),
      generatedAtLabel: formatDateTime(review.generated_at),
      metrics: [
        metric('sessions', 'Sessions', review.resource_counts.sessions),
        metric('active-sessions', 'Active sessions', review.resource_counts.active_sessions),
        metric('projects', 'Projects', review.resource_counts.projects),
        metric('service-principals', 'Service principals', review.resource_counts.service_principals),
        metric('contexts', 'Contexts', review.resource_counts.browser_contexts),
        metric('egress', 'Egress profiles', review.resource_counts.egress_profiles),
        metric('templates', 'Templates', review.resource_counts.session_templates),
        metric('workflows', 'Workflow runs', review.resource_counts.workflow_runs),
        metric('active-workflows', 'Active workflow runs', review.resource_counts.active_workflow_runs),
        metric('automation', 'Automation tasks', review.resource_counts.automation_tasks),
        metric('delegations', 'Delegations', review.resource_counts.delegated_principals),
      ],
      projects: review.projects.map(projectRow),
      servicePrincipals: review.service_principals.map((servicePrincipal) => ({
        id: servicePrincipal.id,
        name: servicePrincipal.name,
        clientId: servicePrincipal.client_id,
        issuer: servicePrincipal.issuer,
        state: servicePrincipal.state,
        scopes: servicePrincipal.scopes.length > 0 ? servicePrincipal.scopes.join(', ') : 'no scopes',
        projects: servicePrincipal.allowed_project_ids.length > 0
          ? servicePrincipal.allowed_project_ids.map(shortId).join(', ')
          : 'all projects metadata unset',
        delegatedSummary: `${servicePrincipal.active_delegated_session_count}/${servicePrincipal.delegated_session_count} active`,
        delegatedSessionIds: servicePrincipal.delegated_session_ids.length > 0
          ? servicePrincipal.delegated_session_ids.map(shortId).join(', ')
          : 'no delegated sessions',
        lastActivity: servicePrincipal.last_delegated_at
          ? `delegated ${formatDateTime(servicePrincipal.last_delegated_at)}`
          : servicePrincipal.last_seen_at
            ? `seen ${formatDateTime(servicePrincipal.last_seen_at)}`
            : 'not observed',
      })),
      delegations: review.delegated_principals.map((delegate) => ({
        clientId: delegate.client_id,
        issuer: delegate.issuer,
        displayName: delegate.display_name ?? delegate.client_id,
        registration: delegate.registered
          ? `registered ${delegate.registered_service_principal_id ? shortId(delegate.registered_service_principal_id) : ''}`.trim()
          : 'unregistered',
        state: delegate.state ?? 'unregistered',
        sessionSummary: `${delegate.active_session_count}/${delegate.session_count} active`,
        sessionIds: delegate.session_ids.length > 0
          ? delegate.session_ids.map(shortId).join(', ')
          : 'no sessions',
      })),
    };
  }
}

function metric(key: string, label: string, value: number): IdentityMetricRow {
  return {
    key,
    label,
    value: String(value),
    testId: `identity-resource-${key}`,
  };
}

function principalTypeLabel(type: IdentityPrincipalType): string {
  switch (type) {
    case 'service_principal':
      return 'Service principal';
    case 'legacy_dev_token':
      return 'Legacy dev token';
    case 'user':
    default:
      return 'User';
  }
}

function projectRow(project: ProjectResource): IdentityProjectRow {
  return {
    id: project.id,
    name: project.name,
    state: project.state,
    activeSessions: quotaLabel(
      project.usage.active_sessions,
      project.usage.max_active_sessions,
    ),
    activeWorkflowRuns: quotaLabel(
      project.usage.active_workflow_runs,
      project.usage.max_active_workflow_runs,
    ),
    retainedStorage: storageLabel(
      project.usage.retained_storage_bytes,
      project.usage.max_retained_storage_bytes,
    ),
  };
}

function quotaLabel(current: number, limit: number | null | undefined): string {
  return limit === null || limit === undefined ? `${current}` : `${current}/${limit}`;
}

function storageLabel(current: number, limit: number | null | undefined): string {
  const currentLabel = formatBytes(current);
  return limit === null || limit === undefined
    ? currentLabel
    : `${currentLabel} / ${formatBytes(limit)}`;
}

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) {
    return '0 B';
  }
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let unitIndex = 0;
  let next = value;
  while (next >= 1024 && unitIndex < units.length - 1) {
    next /= 1024;
    unitIndex += 1;
  }
  return `${next >= 10 || unitIndex === 0 ? next.toFixed(0) : next.toFixed(1)} ${units[unitIndex]}`;
}

function formatDateTime(value: string): string {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleString(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  });
}

function shortId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}
