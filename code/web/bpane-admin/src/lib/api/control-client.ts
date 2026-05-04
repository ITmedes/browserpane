import { ControlSessionMapper } from './control-session-mapper';
import type { CreateSessionCommand, SessionListResponse, SessionResource } from './control-types';

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

  async #request(method: string, path: string, body?: unknown): Promise<unknown> {
    const accessToken = await this.#accessTokenProvider();
    const headers: Record<string, string> = {
      accept: 'application/json',
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
    return await response.json();
  }
}
