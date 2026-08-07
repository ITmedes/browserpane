import {
  AdminApiRequestError,
  AuthenticatedApiClient,
  formatAdminApiRequestError,
  type AccessTokenProvider,
  type AdminApiRequestErrorCode,
  type AdminApiRequestFailure,
  type FetchLike,
} from '$lib/api/authenticated-api';
import type {
  CreateFileWorkspaceRequest,
  FileWorkspaceFileListResponse,
  FileWorkspaceFileResource,
  FileWorkspaceListResponse,
  FileWorkspaceProjectOptionsResponse,
  FileWorkspaceProjectResource,
  FileWorkspaceResource,
  UploadFileWorkspaceFileRequest,
} from './file-workspace-types';

const PROJECT_STATES = ['active', 'archived'] satisfies readonly FileWorkspaceProjectResource['state'][];

export type { AccessTokenProvider, FetchLike } from '$lib/api/authenticated-api';
export type FileWorkspaceCatalogErrorCode = AdminApiRequestErrorCode;

export type FileWorkspaceCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class FileWorkspaceCatalogError extends AdminApiRequestError {
  constructor(
    message: string,
    code: FileWorkspaceCatalogErrorCode,
    status: number | null = null,
    failure?: AdminApiRequestFailure,
  ) {
    super(message, failure ?? { code, status, message });
    this.name = 'FileWorkspaceCatalogError';
  }
}

export class FileWorkspaceCatalogClient {
  readonly #baseUrl: URL;
  readonly #api: AuthenticatedApiClient;

  constructor(options: FileWorkspaceCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#api = new AuthenticatedApiClient({
      baseUrl: this.#baseUrl,
      accessTokenProvider: options.accessTokenProvider,
      ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
      ...(options.onAuthenticationFailure === undefined
        ? {}
        : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) => new FileWorkspaceCatalogError(
        formatAdminApiRequestError('File workspace catalog request', failure),
        failure.code,
        failure.status,
        failure,
      ),
    });
  }

  async listFileWorkspaces(): Promise<FileWorkspaceListResponse> {
    const response = await this.#request(new URL('/api/v1/file-workspaces', this.#baseUrl), {
      method: 'GET',
      headers: { accept: 'application/json' },
    });

    return toFileWorkspaceListResponse(await response.json());
  }

  async createFileWorkspace(request: CreateFileWorkspaceRequest): Promise<FileWorkspaceResource> {
    const response = await this.#request(new URL('/api/v1/file-workspaces', this.#baseUrl), {
      method: 'POST',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    return toFileWorkspaceResource(await response.json());
  }

  async getFileWorkspace(workspaceId: string): Promise<FileWorkspaceResource> {
    const response = await this.#request(
      new URL(`/api/v1/file-workspaces/${encodeURIComponent(workspaceId)}`, this.#baseUrl),
      {
        method: 'GET',
        headers: { accept: 'application/json' },
      },
    );

    return toFileWorkspaceResource(await response.json());
  }

  async listFileWorkspaceFiles(workspaceId: string): Promise<FileWorkspaceFileListResponse> {
    const response = await this.#request(
      new URL(`/api/v1/file-workspaces/${encodeURIComponent(workspaceId)}/files`, this.#baseUrl),
      {
        method: 'GET',
        headers: { accept: 'application/json' },
      },
    );

    return toFileWorkspaceFileListResponse(await response.json());
  }

  async uploadFileWorkspaceFile(
    workspaceId: string,
    request: UploadFileWorkspaceFileRequest,
  ): Promise<FileWorkspaceFileResource> {
    const headers: Record<string, string> = {
      accept: 'application/json',
      'content-type': request.mediaType?.trim() || 'application/octet-stream',
      'x-bpane-file-name': request.fileName,
    };
    if (request.provenance !== undefined && request.provenance !== null) {
      headers['x-bpane-file-provenance'] = JSON.stringify(request.provenance);
    }
    const response = await this.#request(
      new URL(`/api/v1/file-workspaces/${encodeURIComponent(workspaceId)}/files`, this.#baseUrl),
      {
        method: 'POST',
        headers,
        body: request.content,
      },
    );

    return toFileWorkspaceFileResource(await response.json());
  }

  async deleteFileWorkspaceFile(workspaceId: string, fileId: string): Promise<FileWorkspaceFileResource> {
    const response = await this.#request(
      new URL(
        `/api/v1/file-workspaces/${encodeURIComponent(workspaceId)}/files/${encodeURIComponent(fileId)}`,
        this.#baseUrl,
      ),
      {
        method: 'DELETE',
        headers: { accept: 'application/json' },
      },
    );

    return toFileWorkspaceFileResource(await response.json());
  }

  async downloadFileWorkspaceFileContent(file: FileWorkspaceFileResource): Promise<Blob> {
    const response = await this.#request(new URL(file.content_path, this.#baseUrl), {
      method: 'GET',
      headers: { accept: '*/*' },
    });
    return await response.blob();
  }

  async listProjectOptions(): Promise<FileWorkspaceProjectOptionsResponse> {
    const response = await this.#request(new URL('/api/v1/projects', this.#baseUrl), {
      method: 'GET',
      headers: { accept: 'application/json' },
    });

    return toFileWorkspaceProjectOptionsResponse(await response.json());
  }

  async #request(input: URL, init: RequestInit): Promise<Response> {
    return await this.#api.request(input, init);
  }
}

export function toFileWorkspaceListResponse(payload: unknown): FileWorkspaceListResponse {
  const object = expectRecord(payload, 'file workspace list response');
  return {
    workspaces: expectArray(object.workspaces, 'file workspace list workspaces').map(toFileWorkspaceResource),
  };
}

export function toFileWorkspaceFileListResponse(payload: unknown): FileWorkspaceFileListResponse {
  const object = expectRecord(payload, 'file workspace file list response');
  return {
    files: expectArray(object.files, 'file workspace file list files').map(toFileWorkspaceFileResource),
  };
}

export function toFileWorkspaceProjectOptionsResponse(payload: unknown): FileWorkspaceProjectOptionsResponse {
  const object = expectRecord(payload, 'project list response');
  return {
    projects: expectArray(object.projects, 'project list projects').map(toFileWorkspaceProjectResourceFromRequired),
  };
}

export function toFileWorkspaceResource(value: unknown): FileWorkspaceResource {
  const object = expectRecord(value, 'file workspace');
  return {
    id: expectString(object.id, 'file workspace id'),
    project_id: optionalString(object.project_id, 'file workspace project_id') ?? null,
    project: toFileWorkspaceProjectResource(object.project) ?? null,
    name: expectString(object.name, 'file workspace name'),
    description: optionalString(object.description, 'file workspace description') ?? null,
    labels: toStringRecord(object.labels ?? {}, 'file workspace labels'),
    files_path: expectString(object.files_path, 'file workspace files_path'),
    created_at: expectString(object.created_at, 'file workspace created_at'),
    updated_at: expectString(object.updated_at, 'file workspace updated_at'),
  };
}

export function toFileWorkspaceFileResource(value: unknown): FileWorkspaceFileResource {
  const object = expectRecord(value, 'file workspace file');
  return {
    id: expectString(object.id, 'file workspace file id'),
    workspace_id: expectString(object.workspace_id, 'file workspace file workspace_id'),
    name: expectString(object.name, 'file workspace file name'),
    media_type: optionalString(object.media_type, 'file workspace file media_type') ?? null,
    byte_count: expectNumber(object.byte_count, 'file workspace file byte_count'),
    sha256_hex: expectString(object.sha256_hex, 'file workspace file sha256_hex'),
    provenance: toNullableRecord(object.provenance, 'file workspace file provenance'),
    content_path: expectString(object.content_path, 'file workspace file content_path'),
    created_at: expectString(object.created_at, 'file workspace file created_at'),
    updated_at: expectString(object.updated_at, 'file workspace file updated_at'),
  };
}

function toFileWorkspaceProjectResource(value: unknown): FileWorkspaceProjectResource | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return toFileWorkspaceProjectResourceFromRequired(value);
}

function toFileWorkspaceProjectResourceFromRequired(value: unknown): FileWorkspaceProjectResource {
  const object = expectRecord(value, 'file workspace project');
  return {
    id: expectString(object.id, 'file workspace project id'),
    name: expectString(object.name, 'file workspace project name'),
    state: expectEnum(object.state, PROJECT_STATES, 'file workspace project state'),
  };
}

function toNullableRecord(value: unknown, label: string): Readonly<Record<string, unknown>> | null {
  if (value === undefined || value === null) {
    return null;
  }
  return expectRecord(value, label);
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new FileWorkspaceCatalogError(`${label} must be an object.`, 'invalid_payload');
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new FileWorkspaceCatalogError(`${label} must be an array.`, 'invalid_payload');
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string') {
    throw new FileWorkspaceCatalogError(`${label} must be a string.`, 'invalid_payload');
  }
  return value;
}

function optionalString(value: unknown, label: string): string | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return expectString(value, label);
}

function expectNumber(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new FileWorkspaceCatalogError(`${label} must be a finite number.`, 'invalid_payload');
  }
  return value;
}

function expectEnum<T extends string>(value: unknown, allowed: readonly T[], label: string): T {
  const stringValue = expectString(value, label);
  if (!allowed.includes(stringValue as T)) {
    throw new FileWorkspaceCatalogError(
      `${label} must be one of ${allowed.join(', ')}.`,
      'invalid_payload',
    );
  }
  return stringValue as T;
}

function toStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
  const object = expectRecord(value, label);
  return Object.fromEntries(
    Object.entries(object).map(([key, item]) => [key, expectString(item, `${label}.${key}`)]),
  );
}
