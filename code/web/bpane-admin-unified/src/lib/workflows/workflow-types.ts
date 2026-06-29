export type WorkflowDefinitionResource = {
  readonly id: string;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly latest_version?: string | null;
  readonly created_at: string;
  readonly updated_at: string;
};

export type WorkflowDefinitionListResponse = {
  readonly workflows: readonly WorkflowDefinitionResource[];
};

export type WorkflowGitSourceResource = {
  readonly kind: 'git';
  readonly repository_url: string;
  readonly ref?: string | null;
  readonly resolved_commit?: string | null;
  readonly root_path?: string | null;
};

export type WorkflowSourceResource = WorkflowGitSourceResource;

export type WorkflowDefinitionVersionResource = {
  readonly id: string;
  readonly workflow_definition_id: string;
  readonly version: string;
  readonly executor: string;
  readonly entrypoint: string;
  readonly source?: WorkflowSourceResource | null;
  readonly input_schema?: unknown;
  readonly output_schema?: unknown;
  readonly default_session?: unknown;
  readonly allowed_credential_binding_ids: readonly string[];
  readonly allowed_extension_ids: readonly string[];
  readonly allowed_file_workspace_ids: readonly string[];
  readonly created_at: string;
};

export type WorkflowDefinitionVersionListResponse = {
  readonly versions: readonly WorkflowDefinitionVersionResource[];
};

export type WorkflowDefinitionSourcePreviewResource = {
  readonly workflow_definition_id: string;
  readonly workflow_version: string;
  readonly entrypoint: string;
  readonly path: string;
  readonly source: WorkflowSourceResource;
  readonly media_type: string;
  readonly language: string;
  readonly content: string;
  readonly byte_count: number;
  readonly max_bytes: number;
  readonly truncated: boolean;
};

export type WorkflowDefinitionSourceFileResource = {
  readonly path: string;
  readonly byte_count: number;
  readonly media_type: string;
  readonly language: string;
  readonly entrypoint: boolean;
};

export type WorkflowDefinitionSourceFileListResponse = {
  readonly workflow_definition_id: string;
  readonly workflow_version: string;
  readonly entrypoint: string;
  readonly source: WorkflowSourceResource;
  readonly files: readonly WorkflowDefinitionSourceFileResource[];
};

export type ValidateWorkflowDefinitionSourceRequest = {
  readonly entrypoint: string;
  readonly source: WorkflowSourceResource;
};

export type WorkflowDefinitionSourceValidationResponse = {
  readonly workflow_definition_id: string;
  readonly entrypoint: string;
  readonly source: WorkflowSourceResource;
  readonly files: readonly WorkflowDefinitionSourceFileResource[];
};

export type CreateWorkflowDefinitionRequest = {
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
};

export type CreateWorkflowDefinitionVersionRequest = {
  readonly version: string;
  readonly executor: string;
  readonly entrypoint: string;
  readonly source?: unknown;
  readonly input_schema?: unknown;
  readonly output_schema?: unknown;
  readonly default_session?: unknown;
  readonly allowed_credential_binding_ids?: readonly string[];
  readonly allowed_extension_ids?: readonly string[];
  readonly allowed_file_workspace_ids?: readonly string[];
};
