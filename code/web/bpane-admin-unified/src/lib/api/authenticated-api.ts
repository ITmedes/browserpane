export type AccessTokenProvider = () => Promise<string | null> | string | null;
export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export type AuthenticationFailureHandler = (error: AdminApiRequestError) => void;
export type AdminApiRequestErrorCode =
  | 'missing_token'
  | 'http_error'
  | 'network_error'
  | 'request_aborted';

export type AdminApiErrorDetails = {
  readonly message: string;
  readonly code?: string;
  readonly category?: string;
  readonly recoveryHint?: string;
};

export type AdminApiRequestFailure = {
  readonly code: AdminApiRequestErrorCode;
  readonly status: number | null;
  readonly message: string;
  readonly apiCode?: string;
  readonly category?: string;
  readonly recoveryHint?: string;
  readonly cause?: unknown;
};

export type AuthenticatedApiClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider?: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: AuthenticationFailureHandler;
  readonly authentication?: 'required' | 'optional';
  readonly errorFactory?: (failure: AdminApiRequestFailure) => Error;
};

const MAX_ERROR_BODY_LENGTH = 16_384;

export class AdminApiRequestError extends Error {
  readonly status: number | null;
  readonly code: AdminApiRequestErrorCode;
  readonly apiMessage: string;
  readonly apiCode: string | undefined;
  readonly apiCategory: string | undefined;
  readonly recoveryHint: string | undefined;

  constructor(message: string, failure: AdminApiRequestFailure) {
    super(message, failure.cause === undefined ? undefined : { cause: failure.cause });
    this.name = 'AdminApiRequestError';
    this.status = failure.status;
    this.code = failure.code;
    this.apiMessage = failure.message;
    this.apiCode = failure.apiCode;
    this.apiCategory = failure.category;
    this.recoveryHint = failure.recoveryHint;
  }
}

export class AuthenticatedApiClient {
  readonly #baseUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider | undefined;
  readonly #fetchImpl: FetchLike;
  readonly #onAuthenticationFailure: AuthenticationFailureHandler | undefined;
  readonly #authentication: 'required' | 'optional';
  readonly #errorFactory: (failure: AdminApiRequestFailure) => Error;

  constructor(options: AuthenticatedApiClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
    this.#fetchImpl = options.fetchImpl ?? fetch;
    this.#onAuthenticationFailure = options.onAuthenticationFailure;
    this.#authentication = options.authentication ?? 'required';
    this.#errorFactory = options.errorFactory ?? defaultErrorFactory;
  }

  async request(input: string | URL, init: RequestInit = {}): Promise<Response> {
    const headers = new Headers(init.headers);
    const accessToken = await this.#accessTokenProvider?.();
    if (accessToken) {
      headers.set('authorization', `Bearer ${accessToken}`);
    } else if (this.#authentication === 'required') {
      throw this.#errorFactory({
        code: 'missing_token',
        status: null,
        message: 'No active admin access token is available.',
      });
    }

    let response: Response;
    try {
      response = await this.#fetchImpl(this.#resolveUrl(input), {
        ...init,
        headers,
      });
    } catch (cause) {
      throw this.#errorFactory({
        code: isAbortError(cause) ? 'request_aborted' : 'network_error',
        status: null,
        message: isAbortError(cause) ? 'The request was cancelled.' : 'The gateway could not be reached.',
        cause,
      });
    }

    if (response.ok) {
      return response;
    }

    const details = parseAdminApiErrorBody((await safeResponseText(response)).slice(0, MAX_ERROR_BODY_LENGTH));
    const failure: AdminApiRequestFailure = {
      code: 'http_error',
      status: response.status,
      message: details.message,
      ...(details.code === undefined ? {} : { apiCode: details.code }),
      ...(details.category === undefined ? {} : { category: details.category }),
      ...(details.recoveryHint === undefined ? {} : { recoveryHint: details.recoveryHint }),
    };
    const error = this.#errorFactory(failure);
    if (response.status === 401) {
      try {
        this.#onAuthenticationFailure?.(toAdminApiRequestError(error, failure));
      } catch {
        // Authentication recovery must not replace the original request failure.
      }
    }
    throw error;
  }

  #resolveUrl(input: string | URL): URL {
    return input instanceof URL ? input : new URL(input, this.#baseUrl);
  }
}

export function parseAdminApiErrorBody(body: string): AdminApiErrorDetails {
  const trimmed = body.trim();
  if (!trimmed) {
    return { message: '' };
  }

  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return { message: trimmed };
    }
    const record = parsed as Record<string, unknown>;
    return {
      message: typeof record.error === 'string' ? record.error : '',
      ...(typeof record.code === 'string' ? { code: record.code } : {}),
      ...(typeof record.category === 'string' ? { category: record.category } : {}),
      ...(typeof record.recovery_hint === 'string' ? { recoveryHint: record.recovery_hint } : {}),
    };
  } catch {
    return { message: trimmed };
  }
}

export function formatAdminApiRequestError(prefix: string, failure: AdminApiRequestFailure): string {
  if (failure.code === 'missing_token') {
    return failure.message;
  }
  if (failure.code === 'request_aborted') {
    return `${prefix} was cancelled.`;
  }
  if (failure.code === 'network_error') {
    return `${prefix} failed because the gateway could not be reached.`;
  }
  return `${prefix} failed with HTTP ${failure.status ?? 'unknown'}${failure.message ? `: ${failure.message}` : ''}.`;
}

function defaultErrorFactory(failure: AdminApiRequestFailure): Error {
  return new AdminApiRequestError(formatAdminApiRequestError('Admin API request', failure), failure);
}

function toAdminApiRequestError(error: Error, failure: AdminApiRequestFailure): AdminApiRequestError {
  return error instanceof AdminApiRequestError
    ? error
    : new AdminApiRequestError(error.message, failure);
}

function isAbortError(value: unknown): boolean {
  return value instanceof DOMException && value.name === 'AbortError';
}

async function safeResponseText(response: Response): Promise<string> {
  try {
    return await response.text();
  } catch {
    return '';
  }
}
