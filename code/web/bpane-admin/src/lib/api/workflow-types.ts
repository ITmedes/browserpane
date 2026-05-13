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

export type CreateWorkflowDefinitionCommand = {
  readonly name: string;
  readonly description?: string;
  readonly labels?: Readonly<Record<string, string>>;
};

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

export type CreateWorkflowDefinitionVersionCommand = {
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

export type WorkflowRunSessionCommand = {
  readonly existing_session_id?: string;
  readonly create_session?: Record<string, unknown>;
};

export type CreateWorkflowRunCommand = {
  readonly workflow_id: string;
  readonly version: string;
  readonly session?: WorkflowRunSessionCommand;
  readonly input?: unknown;
  readonly source_system?: string;
  readonly source_reference?: string;
  readonly client_request_id?: string;
  readonly labels?: Readonly<Record<string, string>>;
};

export type ResumeWorkflowRunCommand = {
  readonly comment?: string;
  readonly details?: unknown;
};

export type SubmitWorkflowRunInputCommand = {
  readonly input: unknown;
  readonly comment?: string;
  readonly details?: unknown;
};

export type RejectWorkflowRunCommand = {
  readonly reason: string;
  readonly details?: unknown;
};

export type WorkflowRunResource = {
  readonly id: string;
  readonly workflow_definition_id: string;
  readonly workflow_definition_version_id: string;
  readonly workflow_version: string;
  readonly source_system?: string | null;
  readonly source_reference?: string | null;
  readonly client_request_id?: string | null;
  readonly state: string;
  readonly session_id: string;
  readonly automation_task_id: string;
  readonly input?: unknown;
  readonly output?: unknown;
  readonly error?: string | null;
  readonly artifact_refs: readonly string[];
  readonly produced_files: readonly WorkflowRunProducedFileResource[];
  readonly intervention: WorkflowRunInterventionResource;
  readonly runtime?: WorkflowRunRuntimeResource | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly started_at?: string | null;
  readonly completed_at?: string | null;
  readonly events_path: string;
  readonly logs_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type WorkflowRunListResponse = {
  readonly runs: readonly WorkflowRunResource[];
};

export type WorkflowRunInterventionResource = {
  readonly pending_request?: WorkflowRunInterventionRequestResource | null;
};

export type WorkflowRunInterventionRequestResource = {
  readonly request_id: string;
  readonly kind: string;
  readonly prompt?: string | null;
  readonly details?: unknown;
  readonly requested_at: string;
};

export type WorkflowRunRuntimeResource = {
  readonly resume_mode: string;
  readonly exact_runtime_available: boolean;
  readonly hold_until?: string | null;
  readonly released_at?: string | null;
  readonly release_reason?: string | null;
  readonly session_state?: string | null;
};

export type WorkflowRunProducedFileResource = {
  readonly workspace_id: string;
  readonly file_id: string;
  readonly file_name: string;
  readonly media_type?: string | null;
  readonly byte_count: number;
  readonly sha256_hex: string;
  readonly provenance?: unknown;
  readonly content_path: string;
  readonly created_at: string;
};

export type WorkflowRunProducedFileListResponse = {
  readonly files: readonly WorkflowRunProducedFileResource[];
};

export type WorkflowRunEventResource = {
  readonly id: string;
  readonly run_id: string;
  readonly event_type: string;
  readonly message: string;
  readonly source: string;
  readonly automation_task_id?: string | null;
  readonly data?: unknown;
  readonly created_at: string;
};

export type WorkflowRunEventListResponse = {
  readonly events: readonly WorkflowRunEventResource[];
};

export type WorkflowRunLogResource = {
  readonly id: string;
  readonly run_id: string;
  readonly stream: string;
  readonly source: string;
  readonly automation_task_id?: string | null;
  readonly message: string;
  readonly created_at: string;
};

export type WorkflowRunLogListResponse = {
  readonly logs: readonly WorkflowRunLogResource[];
};
