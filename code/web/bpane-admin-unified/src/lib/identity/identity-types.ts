import type { ProjectResource } from '$lib/projects/project-types';

export type ServicePrincipalState = 'active' | 'disabled';

export type ServicePrincipalResource = {
  readonly id: string;
  readonly name: string;
  readonly description: string | null;
  readonly client_id: string;
  readonly issuer: string;
  readonly labels: Readonly<Record<string, string>>;
  readonly scopes: readonly string[];
  readonly allowed_project_ids: readonly string[];
  readonly state: ServicePrincipalState;
  readonly last_seen_at: string | null;
  readonly last_delegated_at: string | null;
  readonly created_at: string;
  readonly updated_at: string;
};

export type ServicePrincipalListResponse = {
  readonly service_principals: readonly ServicePrincipalResource[];
};

export type UpsertServicePrincipalRequest = {
  readonly name: string;
  readonly description?: string | null;
  readonly client_id: string;
  readonly issuer: string;
  readonly labels?: Readonly<Record<string, string>>;
  readonly scopes?: readonly string[];
  readonly allowed_project_ids?: readonly string[];
  readonly state?: ServicePrincipalState;
};

export type IdentityMappingKind = 'user' | 'group' | 'claim' | 'service_principal';
export type IdentityMappingState = 'active' | 'disabled';

export type IdentityMappingResource = {
  readonly id: string;
  readonly name: string;
  readonly description: string | null;
  readonly kind: IdentityMappingKind;
  readonly issuer: string;
  readonly external_id: string;
  readonly claim_name: string | null;
  readonly service_principal_id: string | null;
  readonly project_id: string;
  readonly labels: Readonly<Record<string, string>>;
  readonly scopes: readonly string[];
  readonly state: IdentityMappingState;
  readonly last_seen_at: string | null;
  readonly created_at: string;
  readonly updated_at: string;
};

export type IdentityMappingListResponse = {
  readonly identity_mappings: readonly IdentityMappingResource[];
};

export type UpsertIdentityMappingRequest = {
  readonly name: string;
  readonly description?: string | null;
  readonly kind: IdentityMappingKind;
  readonly issuer: string;
  readonly external_id: string;
  readonly claim_name?: string | null;
  readonly service_principal_id?: string | null;
  readonly project_id: string;
  readonly labels?: Readonly<Record<string, string>>;
  readonly scopes?: readonly string[];
  readonly state?: IdentityMappingState;
};

export type IdentityPrincipalType = 'user' | 'service_principal' | 'legacy_dev_token';

export type IdentityPrincipalResource = {
  readonly subject: string;
  readonly issuer: string;
  readonly display_name: string | null;
  readonly client_id: string | null;
  readonly principal_type: IdentityPrincipalType;
};

export type IdentityResourceCounts = {
  readonly projects: number;
  readonly service_principals: number;
  readonly identity_mappings: number;
  readonly sessions: number;
  readonly active_sessions: number;
  readonly session_templates: number;
  readonly browser_contexts: number;
  readonly egress_profiles: number;
  readonly credential_bindings: number;
  readonly file_workspaces: number;
  readonly workflow_definitions: number;
  readonly workflow_runs: number;
  readonly active_workflow_runs: number;
  readonly automation_tasks: number;
  readonly active_automation_tasks: number;
  readonly extension_definitions: number;
  readonly delegated_principals: number;
};

export type IdentityDelegatedPrincipalResource = {
  readonly client_id: string;
  readonly issuer: string;
  readonly display_name: string | null;
  readonly registered: boolean;
  readonly registered_service_principal_id: string | null;
  readonly state: ServicePrincipalState | null;
  readonly session_count: number;
  readonly active_session_count: number;
  readonly session_ids: readonly string[];
};

export type IdentityServicePrincipalReviewResource = ServicePrincipalResource & {
  readonly delegated_session_count: number;
  readonly active_delegated_session_count: number;
  readonly delegated_session_ids: readonly string[];
};

export type IdentityMappingReviewResource = IdentityMappingResource & {
  readonly effective_for_principal: boolean;
};

export type IdentityUnmappedPrincipalSignalResource = {
  readonly kind: IdentityMappingKind;
  readonly issuer: string;
  readonly external_id: string;
  readonly claim_name: string | null;
  readonly display_name: string | null;
  readonly reason: string;
};

export type IdentityAccessReviewResponse = {
  readonly principal: IdentityPrincipalResource;
  readonly generated_at: string;
  readonly projects: readonly ProjectResource[];
  readonly resource_counts: IdentityResourceCounts;
  readonly identity_mappings: readonly IdentityMappingReviewResource[];
  readonly unmapped_principal_signals: readonly IdentityUnmappedPrincipalSignalResource[];
  readonly service_principals: readonly IdentityServicePrincipalReviewResource[];
  readonly delegated_principals: readonly IdentityDelegatedPrincipalResource[];
};
