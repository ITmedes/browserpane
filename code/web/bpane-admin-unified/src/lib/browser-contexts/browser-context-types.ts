export type BrowserContextState = 'ready' | 'deleted';

export type BrowserContextPersistenceMode = 'reusable' | 'ephemeral';

export type BrowserContextProjectResource = {
  readonly id: string;
  readonly name: string;
  readonly state: 'active' | 'archived';
};

export type BrowserContextUsageResource = {
  readonly visible_session_count: number;
  readonly active_runtime_session_count: number;
  readonly active_runtime_session_id?: string | null;
  readonly profile_storage_bytes?: number | null;
  readonly profile_storage_limit_exceeded: boolean;
};

export type BrowserContextResource = {
  readonly id: string;
  readonly project_id?: string | null;
  readonly project?: BrowserContextProjectResource | null;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly persistence_mode: BrowserContextPersistenceMode;
  readonly retention_sec?: number | null;
  readonly retention_expires_at?: string | null;
  readonly max_profile_storage_bytes?: number | null;
  readonly state: BrowserContextState;
  readonly usage: BrowserContextUsageResource;
  readonly created_at: string;
  readonly updated_at: string;
  readonly last_used_at?: string | null;
  readonly deleted_at?: string | null;
};

export type BrowserContextListResponse = {
  readonly contexts: readonly BrowserContextResource[];
};

export type BrowserContextProjectOptionsResponse = {
  readonly projects: readonly BrowserContextProjectResource[];
};

export type CreateBrowserContextRequest = {
  readonly name: string;
  readonly project_id?: string | null;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly persistence_mode?: BrowserContextPersistenceMode;
  readonly retention_sec?: number | null;
  readonly max_profile_storage_bytes?: number | null;
};

export type CloneBrowserContextRequest = Omit<CreateBrowserContextRequest, 'persistence_mode'>;

export type ImportBrowserContextRequest = Omit<CreateBrowserContextRequest, 'persistence_mode'> & {
  readonly archive: Blob;
};

export type BrowserContextExportArchive = {
  readonly blob: Blob;
  readonly filename: string;
};
