export type AccessTokenProvider = () => Promise<string> | string;
export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export type AuthenticationFailureHandler = (error: ControlApiError) => void;

export type ControlApiErrorDetails = {
  readonly message: string;
  readonly code?: string;
  readonly category?: string;
  readonly recoveryHint?: string;
};

type AuthenticatedRequestOptions = {
  readonly baseUrl: URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl: FetchLike;
  readonly onAuthenticationFailure: AuthenticationFailureHandler | undefined;
  readonly method: string;
  readonly path: string;
  readonly body?: unknown;
  readonly accept?: string;
  readonly bodyMode?: 'json' | 'raw' | undefined;
  readonly contentType?: string | null | undefined;
  readonly headers?: Readonly<Record<string, string>> | undefined;
};

export class ControlApiError extends Error {
  readonly status: number;
  readonly body: string;
  readonly apiMessage: string;
  readonly apiCode: string | undefined;
  readonly apiCategory: string | undefined;
  readonly recoveryHint: string | undefined;

  constructor(
    status: number,
    body: string,
  ) {
    const details = parseControlApiErrorBody(body);
    super(`BrowserPane control API returned HTTP ${status}${details.message ? `: ${details.message}` : ''}`);
    this.status = status;
    this.body = body;
    this.apiMessage = details.message;
    this.apiCode = details.code;
    this.apiCategory = details.category;
    this.recoveryHint = details.recoveryHint;
  }
}

export function parseControlApiErrorBody(body: string): ControlApiErrorDetails {
  const trimmed = body.trim();
  if (!trimmed) {
    return { message: '' };
  }
  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (!parsed || typeof parsed !== 'object') {
      return { message: trimmed };
    }
    const record = parsed as Record<string, unknown>;
    const error = typeof record.error === 'string' ? record.error : trimmed;
    return {
      message: error,
      ...(typeof record.code === 'string' ? { code: record.code } : {}),
      ...(typeof record.category === 'string' ? { category: record.category } : {}),
      ...(typeof record.recovery_hint === 'string' ? { recoveryHint: record.recovery_hint } : {}),
    };
  } catch {
    return { message: trimmed };
  }
}

export async function sendAuthenticatedRequest(options: AuthenticatedRequestOptions): Promise<Response> {
  const accessToken = await options.accessTokenProvider();
  const headers: Record<string, string> = {
    accept: options.accept ?? 'application/json',
    authorization: `Bearer ${accessToken}`,
    ...(options.headers ?? {}),
  };
  const init: RequestInit = { method: options.method, headers };
  if (options.body !== undefined) {
    if (options.bodyMode === 'raw') {
      if (options.contentType !== null) {
        headers['content-type'] = options.contentType ?? 'application/octet-stream';
      }
      init.body = options.body as BodyInit;
    } else {
      headers['content-type'] = options.contentType ?? 'application/json';
      init.body = JSON.stringify(options.body);
    }
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
