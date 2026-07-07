export type FileWorkspaceProjectResource = {
  readonly id: string;
  readonly name: string;
  readonly state: 'active' | 'archived';
};

export type FileWorkspaceResource = {
  readonly id: string;
  readonly project_id?: string | null;
  readonly project?: FileWorkspaceProjectResource | null;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly files_path: string;
  readonly created_at: string;
  readonly updated_at: string;
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

export type FileWorkspaceListResponse = {
  readonly workspaces: readonly FileWorkspaceResource[];
};

export type FileWorkspaceFileListResponse = {
  readonly files: readonly FileWorkspaceFileResource[];
};

export type FileWorkspaceProjectOptionsResponse = {
  readonly projects: readonly FileWorkspaceProjectResource[];
};

export type CreateFileWorkspaceRequest = {
  readonly project_id?: string | null;
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
};

export type UploadFileWorkspaceFileRequest = {
  readonly fileName: string;
  readonly mediaType?: string | null;
  readonly content: BodyInit;
  readonly provenance?: Readonly<Record<string, unknown>> | null;
};
