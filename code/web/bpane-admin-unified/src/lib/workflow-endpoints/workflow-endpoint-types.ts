export type WorkflowEndpointState = 'draft' | 'active' | 'disabled';
export type WorkflowEndpointGrantOperation = 'invoke' | 'read' | 'cancel' | 'artifact.read';

export type WorkflowEndpointArtifactBehavior = {
  readonly mode: 'authorized_references';
  readonly retention_seconds: number;
};

export type WorkflowEndpointResource = {
  readonly id: string;
  readonly project_id: string;
  readonly endpoint_key: string;
  readonly purpose: string;
  readonly workflow_definition_id: string;
  readonly workflow_definition_version_id: string;
  readonly workflow_version: string;
  readonly input_schema: unknown;
  readonly output_schema: unknown;
  readonly execution_timeout_seconds: number;
  readonly inline_result_max_bytes: number;
  readonly artifact_behavior: WorkflowEndpointArtifactBehavior;
  readonly supported_controls: readonly string[];
  readonly labels: Readonly<Record<string, string>>;
  readonly state: WorkflowEndpointState;
  readonly grants_path: string;
  readonly invocations_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type WorkflowEndpointGrantResource = {
  readonly id: string;
  readonly endpoint_id: string;
  readonly project_id: string;
  readonly service_principal_id: string;
  readonly operations: readonly WorkflowEndpointGrantOperation[];
  readonly created_at: string;
  readonly updated_at: string;
};

export type WorkflowEndpointListResponse = {
  readonly workflow_endpoints: readonly WorkflowEndpointResource[];
};

export type WorkflowEndpointGrantListResponse = {
  readonly grants: readonly WorkflowEndpointGrantResource[];
};

export type UpsertWorkflowEndpointRequest = {
  readonly endpoint_key: string;
  readonly purpose: string;
  readonly workflow_definition_id: string;
  readonly workflow_definition_version_id: string;
  readonly workflow_version: string;
  readonly input_schema: unknown;
  readonly output_schema: unknown;
  readonly execution_timeout_seconds: number;
  readonly inline_result_max_bytes: number;
  readonly artifact_behavior: WorkflowEndpointArtifactBehavior;
  readonly labels: Readonly<Record<string, string>>;
};

export type UpsertWorkflowEndpointGrantRequest = {
  readonly service_principal_id: string;
  readonly operations: readonly WorkflowEndpointGrantOperation[];
};

export type WorkflowEndpointOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | {
      readonly status: 'ready';
      readonly projectId: string;
      readonly workflowEndpoints: readonly WorkflowEndpointResource[];
    };

export type WorkflowEndpointDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly projectId: string; readonly endpointKey: string }
  | {
      readonly status: 'error';
      readonly projectId: string;
      readonly endpointKey: string;
      readonly message: string;
    }
  | {
      readonly status: 'ready';
      readonly endpoint: WorkflowEndpointResource;
      readonly grants: readonly WorkflowEndpointGrantResource[];
    };

export type WorkflowEndpointProjectOption = {
  readonly id: string;
  readonly name: string;
  readonly state: string;
};
