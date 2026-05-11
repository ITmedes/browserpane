export type AccessTokenProvider = () => Promise<string> | string;
export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export type AuthenticationFailureHandler = (error: ControlApiError) => void;

type AuthenticatedRequestOptions = {
  readonly baseUrl: URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl: FetchLike;
  readonly onAuthenticationFailure: AuthenticationFailureHandler | undefined;
  readonly method: string;
  readonly path: string;
  readonly body?: unknown;
  readonly accept?: string;
};

export class ControlApiError extends Error {
  constructor(
    readonly status: number,
    readonly body: string,
  ) {
    super(`BrowserPane control API returned HTTP ${status}`);
  }
}

export async function sendAuthenticatedRequest(options: AuthenticatedRequestOptions): Promise<Response> {
  const accessToken = await options.accessTokenProvider();
  const headers: Record<string, string> = {
    accept: options.accept ?? 'application/json',
    authorization: `Bearer ${accessToken}`,
  };
  const init: RequestInit = { method: options.method, headers };
  if (options.body !== undefined) {
    headers['content-type'] = 'application/json';
    init.body = JSON.stringify(options.body);
  }

  const response = await options.fetchImpl(new URL(options.path, options.baseUrl), init);
  if (response.ok) {
    return response;
  }
  const error = new ControlApiError(response.status, await response.text());
  if (response.status === 401) {
    options.onAuthenticationFailure?.(error);
  }
  throw error;
}
