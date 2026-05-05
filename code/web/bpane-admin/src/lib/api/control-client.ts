import { ControlSessionMapper } from './control-session-mapper';
import { ControlSessionFileMapper } from './control-session-file-mapper';
import type {
  CreateSessionCommand,
  SessionAccessTokenResponse,
  SessionFileListResponse,
  SessionFileResource,
  SessionListResponse,
  SessionResource,
  SetAutomationDelegateCommand,
} from './control-types';

export type AccessTokenProvider = () => Promise<string> | string;
export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export type ControlClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
};

export class ControlApiError extends Error {
  constructor(
    readonly status: number,
    readonly body: string,
  ) {
    super(`BrowserPane control API returned HTTP ${status}`);
  }
}

export class ControlClient {
  readonly #baseUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider;
  readonly #fetchImpl: FetchLike;

  constructor(options: ControlClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
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

  async stopSession(sessionId: string): Promise<SessionResource> {
    const payload = await this.#request('POST', `/api/v1/sessions/${encodeURIComponent(sessionId)}/stop`);
    return ControlSessionMapper.toSessionResource(payload);
  }

  async killSession(sessionId: string): Promise<SessionResource> {
    const payload = await this.#request('POST', `/api/v1/sessions/${encodeURIComponent(sessionId)}/kill`);
    return ControlSessionMapper.toSessionResource(payload);
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

  async #request(method: string, path: string, body?: unknown): Promise<unknown> {
    const response = await this.#send(method, path, body, 'application/json');
    return await response.json();
  }

  async #send(method: string, path: string, body?: unknown, accept = 'application/json'): Promise<Response> {
    const accessToken = await this.#accessTokenProvider();
    const headers: Record<string, string> = {
      accept,
      authorization: `Bearer ${accessToken}`,
    };
    const init: RequestInit = {
      method,
      headers,
    };
    if (body !== undefined) {
      headers['content-type'] = 'application/json';
      init.body = JSON.stringify(body);
    }

    const response = await this.#fetchImpl(new URL(path, this.#baseUrl), init);
    if (!response.ok) {
      throw new ControlApiError(response.status, await response.text());
    }
    return response;
  }
}
