import type { ProjectTone } from '$lib/projects/project-formatters';
import { formatDateTime, usageWithLimit } from '$lib/projects/project-formatters';
import type { ProjectResource } from '$lib/projects/project-types';
import { shortIdentityId } from './identity-form-utils';
import type {
  IdentityAccessReviewResponse,
  IdentityMappingKind,
  IdentityPrincipalType,
} from './identity-types';

export type IdentityReviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly review: IdentityAccessReviewResponse };

export type IdentityReviewMetric = {
  readonly id: string;
  readonly label: string;
  readonly value: string;
};

export type IdentityReviewFact = {
  readonly label: string;
  readonly value: string;
};

export type IdentityProjectAccessRow = {
  readonly id: string;
  readonly name: string;
  readonly state: string;
  readonly stateTone: ProjectTone;
  readonly sessions: string;
  readonly workflowRuns: string;
  readonly alerts: string;
};

export type IdentityDelegationRow = {
  readonly key: string;
  readonly name: string;
  readonly clientId: string;
  readonly issuer: string;
  readonly registration: string;
  readonly registrationTone: ProjectTone;
  readonly state: string;
  readonly stateTone: ProjectTone;
  readonly sessions: string;
  readonly sessionIds: string;
};

export type IdentityUnmappedSignalRow = {
  readonly key: string;
  readonly kind: string;
  readonly identity: string;
  readonly issuer: string;
  readonly reason: string;
};

export type IdentityReviewViewModel = {
  readonly principalName: string;
  readonly principalType: string;
  readonly principalFacts: readonly IdentityReviewFact[];
  readonly generatedAt: string;
  readonly metrics: readonly IdentityReviewMetric[];
  readonly projects: readonly IdentityProjectAccessRow[];
  readonly delegations: readonly IdentityDelegationRow[];
  readonly unmappedSignals: readonly IdentityUnmappedSignalRow[];
};

export function buildIdentityReviewViewModel(review: IdentityAccessReviewResponse): IdentityReviewViewModel {
  return {
    principalName: review.principal.display_name ?? review.principal.subject,
    principalType: principalTypeLabel(review.principal.principal_type),
    principalFacts: [
      { label: 'Subject', value: review.principal.subject },
      { label: 'Issuer', value: review.principal.issuer },
      { label: 'Client id', value: review.principal.client_id ?? 'interactive user' },
    ],
    generatedAt: formatDateTime(review.generated_at),
    metrics: [
      metric('projects', 'Projects', review.resource_counts.projects),
      metric(
        'sessions',
        'Active sessions',
        `${review.resource_counts.active_sessions} / ${review.resource_counts.sessions}`,
      ),
      metric(
        'workflow-runs',
        'Active workflow runs',
        `${review.resource_counts.active_workflow_runs} / ${review.resource_counts.workflow_runs}`,
      ),
      metric('service-principals', 'Service principals', review.resource_counts.service_principals),
      metric('identity-mappings', 'Identity mappings', review.resource_counts.identity_mappings),
      metric('delegations', 'Delegated principals', review.resource_counts.delegated_principals),
    ],
    projects: review.projects.map(projectAccessRow),
    delegations: review.delegated_principals.map((principal) => ({
      key: `${principal.issuer}:${principal.client_id}`,
      name: principal.display_name ?? principal.client_id,
      clientId: principal.client_id,
      issuer: principal.issuer,
      registration: principal.registered ? 'registered' : 'unregistered',
      registrationTone: principal.registered ? 'success' : 'warning',
      state: principal.state ?? 'unregistered',
      stateTone: principal.state === 'disabled'
        ? 'danger'
        : principal.state === 'active'
          ? 'success'
          : 'warning',
      sessions: `${principal.active_session_count} / ${principal.session_count} active`,
      sessionIds: principal.session_ids.length > 0
        ? principal.session_ids.map(shortIdentityId).join(', ')
        : 'No delegated sessions',
    })),
    unmappedSignals: review.unmapped_principal_signals.map((signal) => ({
      key: `${signal.kind}:${signal.issuer}:${signal.claim_name ?? ''}:${signal.external_id}`,
      kind: mappingKindLabel(signal.kind),
      identity: signal.claim_name
        ? `${signal.claim_name}=${signal.external_id}`
        : signal.display_name ?? signal.external_id,
      issuer: signal.issuer,
      reason: unmappedReasonLabel(signal.reason),
    })),
  };
}

function metric(id: string, label: string, value: string | number): IdentityReviewMetric {
  return { id, label, value: String(value) };
}

function projectAccessRow(project: ProjectResource): IdentityProjectAccessRow {
  const exceededAlerts = project.usage.alerts.filter((alert) => alert.state === 'exceeded').length;
  return {
    id: project.id,
    name: project.name,
    state: project.state,
    stateTone: project.state === 'active' ? 'success' : 'neutral',
    sessions: usageWithLimit(project.usage.active_sessions, project.usage.max_active_sessions),
    workflowRuns: usageWithLimit(
      project.usage.active_workflow_runs,
      project.usage.max_active_workflow_runs,
    ),
    alerts: project.usage.alerts.length === 0
      ? 'none'
      : exceededAlerts > 0
        ? `${exceededAlerts} exceeded`
        : `${project.usage.alerts.length} warning${project.usage.alerts.length === 1 ? '' : 's'}`,
  };
}

function principalTypeLabel(type: IdentityPrincipalType): string {
  if (type === 'service_principal') {
    return 'Service principal';
  }
  if (type === 'legacy_dev_token') {
    return 'Legacy development token';
  }
  return 'User';
}

export function mappingKindLabel(kind: IdentityMappingKind): string {
  if (kind === 'service_principal') {
    return 'Service principal';
  }
  return `${kind.slice(0, 1).toUpperCase()}${kind.slice(1)}`;
}

function unmappedReasonLabel(reason: string): string {
  const labels: Readonly<Record<string, string>> = {
    current_principal_without_project_mapping: 'Current principal has no active project mapping',
    registered_service_principal_without_project_mapping: 'Registered principal has no active project mapping',
    unregistered_service_principal_without_project_mapping: 'Unregistered principal has no active project mapping',
    group_without_project_mapping: 'Safe group signal has no active project mapping',
    safe_claim_without_project_mapping: 'Safe claim value has no active project mapping',
  };
  return labels[reason] ?? reason.replaceAll('_', ' ');
}
