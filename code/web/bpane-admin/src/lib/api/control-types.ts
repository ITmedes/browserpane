export type SessionRuntimeInfo = {
  readonly binding: string;
  readonly compatibility_mode: string;
  readonly cdp_endpoint?: string | null;
};

export type SessionViewport = {
  readonly width: number;
  readonly height: number;
};

export type BrowserContextState = 'ready' | 'deleted';

export type BrowserContextPersistenceMode = 'reusable' | 'ephemeral';

export type SessionBrowserContextMode = 'fresh' | 'ephemeral' | 'reusable';

export type SessionBrowserContextResource = {
  readonly mode: SessionBrowserContextMode;
  readonly context_id?: string | null;
};

export type SessionBrowserContextCommand = {
  readonly mode: SessionBrowserContextMode;
  readonly context_id?: string | null;
};

export type BrowserContextResource = {
  readonly id: string;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly persistence_mode: BrowserContextPersistenceMode;
  readonly retention_sec?: number | null;
  readonly retention_expires_at?: string | null;
  readonly max_profile_storage_bytes?: number | null;
  readonly state: BrowserContextState;
  readonly usage?: BrowserContextUsageResource | null;
  readonly created_at: string;
  readonly updated_at: string;
  readonly last_used_at?: string | null;
  readonly deleted_at?: string | null;
};

export type BrowserContextUsageResource = {
  readonly visible_session_count: number;
  readonly active_runtime_session_count: number;
  readonly active_runtime_session_id?: string | null;
  readonly profile_storage_bytes?: number | null;
  readonly profile_storage_limit_exceeded: boolean;
};

export type BrowserContextListResponse = {
  readonly contexts: readonly BrowserContextResource[];
};

export type CreateBrowserContextCommand = {
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly persistence_mode?: BrowserContextPersistenceMode;
  readonly retention_sec?: number | null;
  readonly max_profile_storage_bytes?: number | null;
};

export type CloneBrowserContextCommand = {
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly retention_sec?: number | null;
  readonly max_profile_storage_bytes?: number | null;
};

export type SessionConnectInfo = {
  readonly gateway_url: string;
  readonly transport_path: string;
  readonly auth_type: string;
  readonly ticket_path?: string | null;
  readonly compatibility_mode: string;
};

export type SessionStopBlocker = {
  readonly kind: string;
  readonly count: number;
};

export type SessionStopEligibility = {
  readonly allowed: boolean;
  readonly blockers: readonly SessionStopBlocker[];
};

export type SessionAutomationDelegate = {
  readonly client_id: string;
  readonly issuer: string;
  readonly display_name?: string | null;
};

export type SessionConnectionCounts = {
  readonly interactive_clients: number;
  readonly owner_clients: number;
  readonly viewer_clients: number;
  readonly recorder_clients: number;
  readonly automation_clients: number;
  readonly total_clients: number;
};

export type SessionStatusSummary = {
  readonly runtime_state: string;
  readonly runtime_resume_mode: string;
  readonly presence_state: string;
  readonly connection_counts: SessionConnectionCounts;
  readonly stop_eligibility: SessionStopEligibility;
};

export type SessionResource = {
  readonly id: string;
  readonly state: string;
  readonly template_id?: string | null;
  readonly browser_context: SessionBrowserContextResource;
  readonly owner_mode: string;
  readonly viewport?: SessionViewport | null;
  readonly idle_timeout_sec?: number | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly integration_context?: Readonly<Record<string, unknown>> | null;
  readonly automation_delegate?: SessionAutomationDelegate | null;
  readonly connect: SessionConnectInfo;
  readonly runtime: SessionRuntimeInfo;
  readonly status: SessionStatusSummary;
  readonly created_at: string;
  readonly updated_at: string;
  readonly runtime_released_at?: string | null;
  readonly stopped_at?: string | null;
};

export type SessionListResponse = {
  readonly sessions: readonly SessionResource[];
};

export type SessionListFilters = {
  readonly templateId?: string | null;
  readonly states?: readonly string[];
  readonly runtimeStates?: readonly string[];
  readonly labels?: Readonly<Record<string, string>>;
  readonly integrationContext?: Readonly<Record<string, string>>;
  readonly limit?: number | null;
  readonly offset?: number | null;
};

export type SessionTemplateDefaults = {
  readonly owner_mode?: string | null;
  readonly viewport?: SessionViewport | null;
  readonly idle_timeout_sec?: number | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly integration_context?: Readonly<Record<string, unknown>> | null;
  readonly recording?: Readonly<Record<string, unknown>> | null;
};

export type SessionTemplateResource = {
  readonly id: string;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly defaults: SessionTemplateDefaults;
  readonly version: number;
  readonly created_at: string;
  readonly updated_at: string;
};

export type SessionTemplateListResponse = {
  readonly templates: readonly SessionTemplateResource[];
};

export type CreateSessionCommand = {
  readonly template_id?: string | null;
  readonly browser_context?: SessionBrowserContextCommand;
  readonly owner_mode?: string;
  readonly idle_timeout_sec?: number;
  readonly labels?: Readonly<Record<string, string>>;
  readonly integration_context?: Readonly<Record<string, unknown>> | null;
};

export type SetAutomationDelegateCommand = {
  readonly client_id: string;
  readonly issuer?: string | null;
  readonly display_name?: string | null;
};

export type SessionAccessTokenResponse = {
  readonly session_id: string;
  readonly token_type: string;
  readonly token: string;
  readonly expires_at: string;
  readonly connect: SessionConnectInfo;
};

export type SessionFileResource = {
  readonly id: string;
  readonly session_id: string;
  readonly name: string;
  readonly media_type?: string | null;
  readonly byte_count: number;
  readonly sha256_hex: string;
  readonly source: string;
  readonly labels: Readonly<Record<string, string>>;
  readonly content_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type SessionFileListResponse = {
  readonly files: readonly SessionFileResource[];
};

export type FileWorkspaceResource = {
  readonly id: string;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly files_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type FileWorkspaceListResponse = {
  readonly workspaces: readonly FileWorkspaceResource[];
};

export type CreateFileWorkspaceCommand = {
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
};

export type FileWorkspaceFileResource = {
  readonly id: string;
  readonly workspace_id: string;
  readonly name: string;
  readonly media_type?: string | null;
  readonly byte_count: number;
  readonly sha256_hex: string;
  readonly provenance: Readonly<Record<string, unknown>> | null;
  readonly content_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type FileWorkspaceFileListResponse = {
  readonly files: readonly FileWorkspaceFileResource[];
};

export type UploadFileWorkspaceFileCommand = {
  readonly fileName: string;
  readonly content: BodyInit;
  readonly mediaType?: string | null;
  readonly provenance?: Readonly<Record<string, unknown>> | null;
};

export type SessionFileBindingMode = 'read_only' | 'read_write' | 'scratch_output';

export type SessionFileBindingState = 'pending' | 'materialized' | 'failed' | 'removed';

export type SessionFileBindingResource = {
  readonly id: string;
  readonly session_id: string;
  readonly workspace_id: string;
  readonly file_id: string;
  readonly file_name: string;
  readonly media_type?: string | null;
  readonly byte_count: number;
  readonly sha256_hex: string;
  readonly provenance: Readonly<Record<string, unknown>> | null;
  readonly mount_path: string;
  readonly mode: SessionFileBindingMode;
  readonly state: SessionFileBindingState;
  readonly error?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly content_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type SessionFileBindingListResponse = {
  readonly bindings: readonly SessionFileBindingResource[];
};

export type CreateSessionFileBindingCommand = {
  readonly workspace_id: string;
  readonly file_id: string;
  readonly mount_path: string;
  readonly mode?: SessionFileBindingMode;
  readonly labels?: Readonly<Record<string, string>>;
};
