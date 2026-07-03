import type {
  ProjectAdmissionDecision,
  SessionProjectResource,
} from '$lib/sessions/session-types';

export type WorkflowRunResource = {
  readonly id: string;
  readonly workflow_definition_id: string;
  readonly workflow_definition_version_id: string;
  readonly workflow_version: string;
  readonly project_id?: string | null;
  readonly project?: SessionProjectResource | null;
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
  readonly project_admission?: ProjectAdmissionDecision | null;
  readonly admission?: WorkflowRunAdmissionResource | null;
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

export type WorkflowRunSessionBindingRequest = {
  readonly existing_session_id?: string;
  readonly create_session?: unknown;
};

export type CreateWorkflowRunWorkspaceInputRequest = {
  readonly workspace_id: string;
  readonly file_id: string;
  readonly mount_path?: string | null;
};

export type CreateWorkflowRunRequest = {
  readonly workflow_id: string;
  readonly version: string;
  readonly project_id?: string | null;
  readonly session?: WorkflowRunSessionBindingRequest;
  readonly input?: unknown;
  readonly source_system?: string | null;
  readonly source_reference?: string | null;
  readonly client_request_id?: string | null;
  readonly credential_binding_ids?: readonly string[];
  readonly workspace_inputs?: readonly CreateWorkflowRunWorkspaceInputRequest[];
  readonly labels?: Readonly<Record<string, string>>;
};

export type WorkflowRunLaunchResult = {
  readonly run: WorkflowRunResource;
  readonly previewOpened: boolean;
  readonly previewBlocked?: boolean;
};

export type WorkflowRunAdmissionResource = {
  readonly state: string;
  readonly reason: string;
  readonly details?: unknown;
  readonly queued_at: string;
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
