export type SessionFileSource = 'browser_upload' | 'browser_download';

export type SessionFileResource = {
  readonly id: string;
  readonly session_id: string;
  readonly name: string;
  readonly media_type?: string | null;
  readonly byte_count: number;
  readonly sha256_hex: string;
  readonly source: SessionFileSource;
  readonly labels: Readonly<Record<string, string>>;
  readonly content_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type SessionFileListResponse = {
  readonly files: readonly SessionFileResource[];
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

export type CreateSessionFileBindingRequest = {
  readonly workspace_id: string;
  readonly file_id: string;
  readonly mount_path: string;
  readonly mode?: SessionFileBindingMode;
  readonly labels?: Readonly<Record<string, string>>;
};
