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
  CreateSessionCommand,
  CreateFileWorkspaceCommand,
  CreateSessionFileBindingCommand,
  FileWorkspaceFileListResponse,
  FileWorkspaceFileResource,
  FileWorkspaceListResponse,
  FileWorkspaceResource,
  SessionAccessTokenResponse,
  SessionFileBindingListResponse,
  SessionFileBindingResource,
  SessionFileListResponse,
  SessionFileResource,
  SessionListResponse,
  SessionResource,
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

  async listSessions(): Promise<SessionListResponse> {
    const payload = await this.#request('GET', '/api/v1/sessions');
    return ControlSessionMapper.toSessionList(payload);
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

  async stopSession(sessionId: string): Promise<SessionResource> {
    const payload = await this.#request('POST', `/api/v1/sessions/${encodeURIComponent(sessionId)}/stop`);
    return ControlSessionMapper.toSessionResource(payload);
  }

  async killSession(sessionId: string): Promise<SessionResource> {
    const payload = await this.#request('POST', `/api/v1/sessions/${encodeURIComponent(sessionId)}/kill`);
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
