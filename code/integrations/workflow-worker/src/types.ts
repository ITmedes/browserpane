export type GatewaySessionAutomationAccessResponse = {
  session_id: string;
  token_type: "session_automation_access_token";
  token: string;
  expires_at: string;
  automation: {
    endpoint_url: string;
    protocol: "chrome_devtools_protocol";
    auth_type: "session_automation_access_token";
    auth_header: string;
    status_path: string;
    mcp_owner_path: string;
    compatibility_mode: string;
  };
};

export type GatewayWorkflowRunState =
  | "pending"
  | "queued"
  | "starting"
  | "running"
  | "awaiting_input"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "timed_out";

export type GatewayAutomationTaskLogStream = "stdout" | "stderr" | "system";

export type GatewayWorkflowSource = {
  kind: "git";
  repository_url: string;
  ref: string | null;
  resolved_commit: string | null;
  root_path: string | null;
};

export type GatewayWorkflowRunSourceSnapshot = {
  source: GatewayWorkflowSource;
  entrypoint: string;
  workspace_id: string;
  file_id: string;
  file_name: string;
  media_type: string | null;
  content_path: string;
};

export type GatewayCredentialInjectionMode =
  | "form_fill"
  | "cookie_seed"
  | "storage_seed"
  | "totp_fill";

export type GatewayCredentialTotpMetadata = {
  issuer: string | null;
  account_name: string | null;
  period_sec: number | null;
  digits: number | null;
};

export type GatewayWorkflowRunCredentialBinding = {
  id: string;
  name: string;
  provider: "vault_kv_v2";
  namespace: string | null;
  allowed_origins: string[];
  injection_mode: GatewayCredentialInjectionMode;
  totp: GatewayCredentialTotpMetadata | null;
  resolve_path: string;
};

export type GatewayResolvedWorkflowRunCredentialBinding = {
  binding: GatewayWorkflowRunCredentialBinding;
  payload: unknown;
};

export type GatewayWorkflowRunWorkspaceInput = {
  id: string;
  workspace_id: string;
  file_id: string;
  file_name: string;
  media_type: string | null;
  byte_count: number;
  sha256_hex: string;
  provenance: unknown;
  mount_path: string;
  content_path: string;
};

export type GatewayWorkflowRunProducedFile = {
  workspace_id: string;
  file_id: string;
  file_name: string;
  media_type: string | null;
  byte_count: number;
  sha256_hex: string;
  provenance: unknown;
  content_path: string;
  created_at: string;
};

export type GatewayWorkflowRunRecording = {
  id: string;
  session_id: string;
  state: string;
  format: string;
  mime_type: string | null;
  bytes: number | null;
  duration_ms: number | null;
  error: string | null;
  termination_reason: string | null;
  previous_recording_id: string | null;
  started_at: string;
  completed_at: string | null;
  content_path: string;
  created_at: string;
  updated_at: string;
};

export type GatewayWorkflowRunRetention = {
  logs_expire_at: string | null;
  output_expire_at: string | null;
};

export type GatewayWorkflowRunAdmission = {
  state: "queued";
  reason: string;
  details: unknown;
  queued_at: string;
};

export type GatewayWorkflowRunResource = {
  id: string;
  workflow_definition_id: string;
  workflow_definition_version_id: string;
  workflow_version: string;
  state: GatewayWorkflowRunState;
  session_id: string;
  automation_task_id: string;
  input: unknown;
  output: unknown;
  error: string | null;
  artifact_refs: string[];
  source_snapshot: GatewayWorkflowRunSourceSnapshot | null;
  credential_bindings: GatewayWorkflowRunCredentialBinding[];
  workspace_inputs: GatewayWorkflowRunWorkspaceInput[];
  produced_files: GatewayWorkflowRunProducedFile[];
  recordings: GatewayWorkflowRunRecording[];
  retention: GatewayWorkflowRunRetention;
  admission: GatewayWorkflowRunAdmission | null;
  labels: Record<string, string>;
  started_at: string | null;
  completed_at: string | null;
  events_path: string;
  logs_path: string;
  created_at: string;
  updated_at: string;
};

export type GatewayWorkflowRunProducedFileResource = GatewayWorkflowRunProducedFile;

export type GatewayWorkflowDefinitionVersionResource = {
  id: string;
  workflow_definition_id: string;
  version: string;
  executor: string;
  entrypoint: string;
  source: GatewayWorkflowSource | null;
  input_schema: unknown;
  output_schema: unknown;
  default_session: unknown;
  allowed_credential_binding_ids: string[];
  allowed_extension_ids: string[];
  allowed_file_workspace_ids: string[];
  created_at: string;
};

export type WorkflowEntrypointResult = {
  output: unknown;
};

export type WorkflowRunnerContext = {
  gatewayApiUrl: string;
  endpointUrl: string;
  authHeader: string;
  authToken: string;
  entrypointPath: string;
  sourceRoot: string;
  credentialBindings: WorkflowRunnerCredentialBinding[];
  credentialBindingFiles: Record<string, string>;
  workspaceInputs: WorkflowRunnerWorkspaceInput[];
  input: unknown;
  sessionId: string;
  workflowRunId: string;
  automationTaskId: string;
  resultPath: string;
};

export type WorkflowRunnerWorkspaceInput = {
  id: string;
  workspaceId: string;
  fileId: string;
  fileName: string;
  mediaType: string | null;
  byteCount: number;
  sha256Hex: string;
  provenance: unknown;
  mountPath: string;
  localPath: string;
};

export type WorkflowRunnerCredentialBinding = {
  id: string;
  name: string;
  provider: "vault_kv_v2";
  namespace: string | null;
  allowedOrigins: string[];
  injectionMode: GatewayCredentialInjectionMode;
  totp: GatewayCredentialTotpMetadata | null;
};

export type WorkflowResolvedCredentialBinding = WorkflowRunnerCredentialBinding & {
  payload: unknown;
};

export type WorkflowProducedFileUploadRequest = {
  workspaceId: string;
  fileName: string;
  bytes: Uint8Array;
  mediaType?: string | null;
  provenance?: Record<string, unknown> | null;
};
