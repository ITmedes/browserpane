import { ControlSessionMapper } from './control-session-mapper';
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
  SessionAccessTokenResponse,
  SessionFileListResponse,
  SessionFileResource,
  SessionListResponse,
  SessionResource,
  SetAutomationDelegateCommand,
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

  async #send(method: string, path: string, body?: unknown, accept = 'application/json'): Promise<Response> {
    return await sendAuthenticatedRequest({
      baseUrl: this.#baseUrl,
      accessTokenProvider: this.#accessTokenProvider,
      fetchImpl: this.#fetchImpl,
      onAuthenticationFailure: this.#onAuthenticationFailure,
      method,
      path,
      body,
      accept,
    });
  }
}
