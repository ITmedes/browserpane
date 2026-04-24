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
  workspace_inputs: GatewayWorkflowRunWorkspaceInput[];
  labels: Record<string, string>;
  started_at: string | null;
  completed_at: string | null;
  events_path: string;
  logs_path: string;
  created_at: string;
  updated_at: string;
};

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
  endpointUrl: string;
  authHeader: string;
  authToken: string;
  entrypointPath: string;
  sourceRoot: string;
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
