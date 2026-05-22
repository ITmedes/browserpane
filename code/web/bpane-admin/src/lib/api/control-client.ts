import { ControlSessionMapper } from './control-session-mapper';
import { ControlFileWorkspaceMapper } from './control-file-workspace-mapper';
import { ControlSessionFileMapper } from './control-session-file-mapper';
import { ControlSessionStatusMapper } from './control-session-status-mapper';
import { RecordingMapper } from './recording-mapper';
import {
  sendAuthenticatedRequest,
  type AccessTokenProvider,
  type AuthenticationFailureHandler,
  type FetchLike,
} from './authenticated-api';
import type {
  BrowserContextListResponse,
  BrowserContextResource,
  CloneBrowserContextCommand,
  CreateBrowserContextCommand,
  CreateEgressProfileCommand,
  ImportBrowserContextCommand,
  CreateSessionCommand,
  CreateFileWorkspaceCommand,
  CreateSessionFileBindingCommand,
  EgressDiagnosticsResource,
  EgressProfileListResponse,
  EgressProfileResource,
  FileWorkspaceFileListResponse,
  FileWorkspaceFileResource,
  FileWorkspaceListResponse,
  FileWorkspaceResource,
  SessionAccessTokenResponse,
  SessionFileBindingListResponse,
  SessionFileBindingResource,
  SessionFileListResponse,
  SessionFileResource,
  SessionListFilters,
  SessionListResponse,
  SessionResource,
  SessionTemplateListResponse,
  SetAutomationDelegateCommand,
  UploadFileWorkspaceFileCommand,
} from './control-types';
import type {
  SessionRecordingListResponse,
  SessionRecordingPlaybackResource,
  SessionRecordingResource,
} from './recording-types';
import type { SessionStatus } from './session-status-types';

export {
  ControlApiError,
  type AccessTokenProvider,
  type AuthenticationFailureHandler,
  type FetchLike,
} from './authenticated-api';

export type ControlClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly onAuthenticationFailure?: AuthenticationFailureHandler;
  readonly fetchImpl?: FetchLike;
};

export class ControlClient {
  readonly #baseUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider;
  readonly #onAuthenticationFailure: AuthenticationFailureHandler | undefined;
  readonly #fetchImpl: FetchLike;

  constructor(options: ControlClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
    this.#onAuthenticationFailure = options.onAuthenticationFailure;
    this.#fetchImpl = options.fetchImpl ?? fetch;
  }

  async listSessions(filters: SessionListFilters = {}): Promise<SessionListResponse> {
    const payload = await this.#request('GET', buildSessionListPath(filters));
    return ControlSessionMapper.toSessionList(payload);
  }

  async listSessionTemplates(): Promise<SessionTemplateListResponse> {
    const payload = await this.#request('GET', '/api/v1/session-templates');
    return ControlSessionMapper.toSessionTemplateList(payload);
  }

  async listEgressProfiles(): Promise<EgressProfileListResponse> {
    const payload = await this.#request('GET', '/api/v1/egress-profiles');
    return ControlSessionMapper.toEgressProfileList(payload);
  }

  async createEgressProfile(command: CreateEgressProfileCommand): Promise<EgressProfileResource> {
    const payload = await this.#request('POST', '/api/v1/egress-profiles', {
      ...command,
      labels: command.labels ?? {},
      bypass_rules: command.bypass_rules ?? [],
    });
    return ControlSessionMapper.toEgressProfileResource(payload);
  }

  async getEgressProfile(profileId: string): Promise<EgressProfileResource> {
    const payload = await this.#request('GET', `/api/v1/egress-profiles/${encodeURIComponent(profileId)}`);
    return ControlSessionMapper.toEgressProfileResource(payload);
  }

  async getEgressProfileDiagnostics(profileId: string): Promise<EgressDiagnosticsResource> {
    const payload = await this.#request('GET', `/api/v1/egress-profiles/${encodeURIComponent(profileId)}/diagnostics`);
    return ControlSessionMapper.toEgressDiagnosticsResource(payload);
  }

  async updateEgressProfile(profileId: string, command: CreateEgressProfileCommand): Promise<EgressProfileResource> {
    const payload = await this.#request('PUT', `/api/v1/egress-profiles/${encodeURIComponent(profileId)}`, {
      ...command,
      labels: command.labels ?? {},
      bypass_rules: command.bypass_rules ?? [],
    });
    return ControlSessionMapper.toEgressProfileResource(payload);
  }

  async listBrowserContexts(): Promise<BrowserContextListResponse> {
    const payload = await this.#request('GET', '/api/v1/browser-contexts');
    return ControlSessionMapper.toBrowserContextList(payload);
  }

  async createBrowserContext(command: CreateBrowserContextCommand): Promise<BrowserContextResource> {
    const payload = await this.#request('POST', '/api/v1/browser-contexts', {
      ...command,
      labels: command.labels ?? {},
    });
    return ControlSessionMapper.toBrowserContextResource(payload);
  }

  async getBrowserContext(contextId: string): Promise<BrowserContextResource> {
    const payload = await this.#request('GET', `/api/v1/browser-contexts/${encodeURIComponent(contextId)}`);
    return ControlSessionMapper.toBrowserContextResource(payload);
  }

  async cloneBrowserContext(
    contextId: string,
    command: CloneBrowserContextCommand,
  ): Promise<BrowserContextResource> {
    const payload = await this.#request('POST', `/api/v1/browser-contexts/${encodeURIComponent(contextId)}/clone`, {
      ...command,
      labels: command.labels ?? undefined,
    });
    return ControlSessionMapper.toBrowserContextResource(payload);
  }

  async exportBrowserContext(contextId: string): Promise<Blob> {
    const response = await this.#send(
      'GET',
      `/api/v1/browser-contexts/${encodeURIComponent(contextId)}/export`,
      undefined,
      'application/zip',
    );
    return await response.blob();
  }

  async importBrowserContext(command: ImportBrowserContextCommand): Promise<BrowserContextResource> {
    const headers: Record<string, string> = {
      'x-bpane-browser-context-name': command.name,
    };
    if (command.description !== undefined && command.description !== null) {
      headers['x-bpane-browser-context-description'] = command.description;
    }
    if (command.labels !== undefined && command.labels !== null) {
      headers['x-bpane-browser-context-labels'] = JSON.stringify(command.labels);
    }
    if (command.retention_sec !== undefined && command.retention_sec !== null) {
      headers['x-bpane-browser-context-retention-sec'] = String(command.retention_sec);
    }
    if (command.max_profile_storage_bytes !== undefined && command.max_profile_storage_bytes !== null) {
      headers['x-bpane-browser-context-max-profile-storage-bytes'] = String(command.max_profile_storage_bytes);
    }
    const response = await this.#send(
      'POST',
      '/api/v1/browser-contexts/import',
      command.archive,
      'application/json',
      {
        bodyMode: 'raw',
        contentType: 'application/zip',
        headers,
      },
    );
    return ControlSessionMapper.toBrowserContextResource(await response.json());
  }

  async deleteBrowserContext(contextId: string): Promise<BrowserContextResource> {
    const payload = await this.#request('DELETE', `/api/v1/browser-contexts/${encodeURIComponent(contextId)}`);
    return ControlSessionMapper.toBrowserContextResource(payload);
  }

  async createSession(command: CreateSessionCommand = {}): Promise<SessionResource> {
    const payload = await this.#request('POST', '/api/v1/sessions', command);
    return ControlSessionMapper.toSessionResource(payload);
  }

  async getSession(sessionId: string): Promise<SessionResource> {
    const payload = await this.#request('GET', `/api/v1/sessions/${encodeURIComponent(sessionId)}`);
    return ControlSessionMapper.toSessionResource(payload);
  }

  async getSessionStatus(sessionId: string): Promise<SessionStatus> {
    const payload = await this.#request('GET', `/api/v1/sessions/${encodeURIComponent(sessionId)}/status`);
    return ControlSessionStatusMapper.toSessionStatus(payload);
  }

  async getSessionEgressDiagnostics(sessionId: string): Promise<EgressDiagnosticsResource> {
    const payload = await this.#request('GET', `/api/v1/sessions/${encodeURIComponent(sessionId)}/egress-diagnostics`);
    return ControlSessionMapper.toEgressDiagnosticsResource(payload);
  }

  async stopSession(sessionId: string): Promise<SessionResource> {
    const payload = await this.#request('POST', `/api/v1/sessions/${encodeURIComponent(sessionId)}/stop`);
    return ControlSessionMapper.toSessionResource(payload);
  }

  async killSession(sessionId: string): Promise<SessionResource> {
    const payload = await this.#request('POST', `/api/v1/sessions/${encodeURIComponent(sessionId)}/kill`);
    return ControlSessionMapper.toSessionResource(payload);
  }

  async releaseSessionRuntime(sessionId: string): Promise<SessionResource> {
    const payload = await this.#request('POST', `/api/v1/sessions/${encodeURIComponent(sessionId)}/release`);
    return ControlSessionMapper.toSessionResource(payload);
  }

  async disconnectSessionConnection(sessionId: string, connectionId: number): Promise<SessionStatus> {
    const payload = await this.#request(
      'POST',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/connections/${connectionId}/disconnect`,
    );
    return ControlSessionStatusMapper.toSessionStatus(payload);
  }

  async disconnectAllSessionConnections(sessionId: string): Promise<SessionStatus> {
    const payload = await this.#request('POST', `/api/v1/sessions/${encodeURIComponent(sessionId)}/connections/disconnect-all`);
    return ControlSessionStatusMapper.toSessionStatus(payload);
  }

  async setAutomationDelegate(
    sessionId: string,
    command: SetAutomationDelegateCommand,
  ): Promise<SessionResource> {
    const payload = await this.#request(
      'POST',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-owner`,
      command,
    );
    return ControlSessionMapper.toSessionResource(payload);
  }

  async clearAutomationDelegate(sessionId: string): Promise<SessionResource> {
    const payload = await this.#request(
      'DELETE',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-owner`,
    );
    return ControlSessionMapper.toSessionResource(payload);
  }

  async issueSessionAccessToken(sessionId: string): Promise<SessionAccessTokenResponse> {
    const payload = await this.#request(
      'POST',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/access-tokens`,
    );
    return ControlSessionMapper.toSessionAccessTokenResponse(payload);
  }

  async listSessionFiles(sessionId: string): Promise<SessionFileListResponse> {
    const payload = await this.#request(
      'GET',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/files`,
    );
    return ControlSessionFileMapper.toSessionFileList(payload);
  }

  async getSessionFile(sessionId: string, fileId: string): Promise<SessionFileResource> {
    const payload = await this.#request(
      'GET',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/files/${encodeURIComponent(fileId)}`,
    );
    return ControlSessionFileMapper.toSessionFileResource(payload);
  }

  async downloadSessionFileContent(file: SessionFileResource): Promise<Blob> {
    const response = await this.#send('GET', file.content_path, undefined, '*/*');
    return await response.blob();
  }

  async listFileWorkspaces(): Promise<FileWorkspaceListResponse> {
    const payload = await this.#request('GET', '/api/v1/file-workspaces');
    return ControlFileWorkspaceMapper.toWorkspaceList(payload);
  }

  async createFileWorkspace(command: CreateFileWorkspaceCommand): Promise<FileWorkspaceResource> {
    const payload = await this.#request('POST', '/api/v1/file-workspaces', {
      ...command,
      labels: command.labels ?? {},
    });
    return ControlFileWorkspaceMapper.toWorkspace(payload);
  }

  async getFileWorkspace(workspaceId: string): Promise<FileWorkspaceResource> {
    const payload = await this.#request(
      'GET',
      `/api/v1/file-workspaces/${encodeURIComponent(workspaceId)}`,
    );
    return ControlFileWorkspaceMapper.toWorkspace(payload);
  }

  async listFileWorkspaceFiles(workspaceId: string): Promise<FileWorkspaceFileListResponse> {
    const payload = await this.#request(
      'GET',
      `/api/v1/file-workspaces/${encodeURIComponent(workspaceId)}/files`,
    );
    return ControlFileWorkspaceMapper.toWorkspaceFileList(payload);
  }

  async uploadFileWorkspaceFile(
    workspaceId: string,
    command: UploadFileWorkspaceFileCommand,
  ): Promise<FileWorkspaceFileResource> {
    const headers: Record<string, string> = {
      'x-bpane-file-name': command.fileName,
    };
    if (command.provenance !== undefined && command.provenance !== null) {
      headers['x-bpane-file-provenance'] = JSON.stringify(command.provenance);
    }
    const response = await this.#send(
      'POST',
      `/api/v1/file-workspaces/${encodeURIComponent(workspaceId)}/files`,
      command.content,
      'application/json',
      {
        bodyMode: 'raw',
        contentType: command.mediaType ?? 'application/octet-stream',
        headers,
      },
    );
    return ControlFileWorkspaceMapper.toWorkspaceFile(await response.json());
  }

  async getFileWorkspaceFile(
    workspaceId: string,
    fileId: string,
  ): Promise<FileWorkspaceFileResource> {
    const payload = await this.#request(
      'GET',
      `/api/v1/file-workspaces/${encodeURIComponent(workspaceId)}/files/${encodeURIComponent(fileId)}`,
    );
    return ControlFileWorkspaceMapper.toWorkspaceFile(payload);
  }

  async deleteFileWorkspaceFile(
    workspaceId: string,
    fileId: string,
  ): Promise<FileWorkspaceFileResource> {
    const payload = await this.#request(
      'DELETE',
      `/api/v1/file-workspaces/${encodeURIComponent(workspaceId)}/files/${encodeURIComponent(fileId)}`,
    );
    return ControlFileWorkspaceMapper.toWorkspaceFile(payload);
  }

  async downloadFileWorkspaceFileContent(file: FileWorkspaceFileResource): Promise<Blob> {
    const response = await this.#send('GET', file.content_path, undefined, '*/*');
    return await response.blob();
  }

  async listSessionFileBindings(sessionId: string): Promise<SessionFileBindingListResponse> {
    const payload = await this.#request(
      'GET',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/file-bindings`,
    );
    return ControlFileWorkspaceMapper.toSessionFileBindingList(payload);
  }

  async createSessionFileBinding(
    sessionId: string,
    command: CreateSessionFileBindingCommand,
  ): Promise<SessionFileBindingResource> {
    const payload = await this.#request(
      'POST',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/file-bindings`,
      {
        ...command,
        labels: command.labels ?? {},
      },
    );
    return ControlFileWorkspaceMapper.toSessionFileBinding(payload);
  }

  async getSessionFileBinding(
    sessionId: string,
    bindingId: string,
  ): Promise<SessionFileBindingResource> {
    const payload = await this.#request(
      'GET',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/file-bindings/${encodeURIComponent(bindingId)}`,
    );
    return ControlFileWorkspaceMapper.toSessionFileBinding(payload);
  }

  async removeSessionFileBinding(
    sessionId: string,
    bindingId: string,
  ): Promise<SessionFileBindingResource> {
    const payload = await this.#request(
      'DELETE',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/file-bindings/${encodeURIComponent(bindingId)}`,
    );
    return ControlFileWorkspaceMapper.toSessionFileBinding(payload);
  }

  async downloadSessionFileBindingContent(binding: SessionFileBindingResource): Promise<Blob> {
    const response = await this.#send('GET', binding.content_path, undefined, '*/*');
    return await response.blob();
  }

  async listSessionRecordings(sessionId: string): Promise<SessionRecordingListResponse> {
    const payload = await this.#request(
      'GET',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/recordings`,
    );
    return RecordingMapper.toRecordingList(payload);
  }

  async getSessionRecordingPlayback(sessionId: string): Promise<SessionRecordingPlaybackResource> {
    const payload = await this.#request(
      'GET',
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/recording-playback`,
    );
    return RecordingMapper.toPlayback(payload);
  }

  async downloadSessionRecordingContent(recording: SessionRecordingResource): Promise<Blob> {
    const response = await this.#send('GET', recording.content_path, undefined, '*/*');
    return await response.blob();
  }

  async downloadSessionRecordingPlaybackExport(
    playback: SessionRecordingPlaybackResource,
  ): Promise<Blob> {
    const response = await this.#send('GET', playback.export_path, undefined, 'application/zip');
    return await response.blob();
  }

  async #request(method: string, path: string, body?: unknown): Promise<unknown> {
    const response = await this.#send(method, path, body, 'application/json');
    return await response.json();
  }

  async #send(
    method: string,
    path: string,
    body?: unknown,
    accept = 'application/json',
    options: {
      readonly bodyMode?: 'json' | 'raw';
      readonly contentType?: string | null;
      readonly headers?: Readonly<Record<string, string>>;
    } = {},
  ): Promise<Response> {
    return await sendAuthenticatedRequest({
      baseUrl: this.#baseUrl,
      accessTokenProvider: this.#accessTokenProvider,
      fetchImpl: this.#fetchImpl,
      onAuthenticationFailure: this.#onAuthenticationFailure,
      method,
      path,
      body,
      accept,
      bodyMode: options.bodyMode,
      contentType: options.contentType,
      headers: options.headers,
    });
  }
}

function buildSessionListPath(filters: SessionListFilters): string {
  const params = new URLSearchParams();
  if (filters.templateId) {
    params.set('template_id', filters.templateId);
  }
  appendCsvFilter(params, 'state', filters.states);
  appendCsvFilter(params, 'runtime_state', filters.runtimeStates);
  for (const [key, value] of Object.entries(filters.labels ?? {})) {
    params.append(`label.${key}`, value);
  }
  for (const [key, value] of Object.entries(filters.integrationContext ?? {})) {
    params.append(`integration.${key}`, value);
  }
  if (filters.limit !== undefined && filters.limit !== null) {
    params.set('limit', String(filters.limit));
  }
  if (filters.offset !== undefined && filters.offset !== null) {
    params.set('offset', String(filters.offset));
  }
  const query = params.toString();
  return query ? `/api/v1/sessions?${query}` : '/api/v1/sessions';
}

function appendCsvFilter(
  params: URLSearchParams,
  key: string,
  values: readonly string[] | undefined,
): void {
  const filtered = values?.map((value) => value.trim()).filter(Boolean) ?? [];
  if (filtered.length > 0) {
    params.set(key, filtered.join(','));
  }
}
