import {
  AdminEventStreamAccessMapper,
  AdminEventStreamClient,
  type AdminEventConnectionStatus,
  type AdminEventStreamAccess,
  type AdminEventSubscription,
  type AdminEventWebSocket,
  type AdminEventWebSocketFactory,
} from '@browserpane/admin-auth';
import {
  AuthenticatedApiClient,
  type AccessTokenProvider,
  type AuthenticationFailureHandler,
  type FetchLike,
} from './authenticated-api';
import { toAdminEvent, type AdminEvent } from './admin-events';

export type {
  AdminEventConnectionStatus,
  AdminEventSubscription,
  AdminEventWebSocket,
  AdminEventWebSocketFactory,
};

export type AdminEventClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly onAuthenticationFailure?: AuthenticationFailureHandler;
  readonly fetchImpl?: FetchLike;
  readonly webSocketFactory?: AdminEventWebSocketFactory;
  readonly reconnectDelayMs?: number;
};

export type AdminEventHandlers = {
  readonly onEvent: (event: AdminEvent) => void;
  readonly onStatus?: (status: AdminEventConnectionStatus) => void;
  readonly onError?: (error: Error) => void;
};

export class AdminEventClient {
  readonly #api: AuthenticatedApiClient;
  readonly #stream: AdminEventStreamClient<AdminEvent>;

  constructor(options: AdminEventClientOptions) {
    const baseUrl = new URL(options.baseUrl);
    this.#api = new AuthenticatedApiClient({
      baseUrl,
      accessTokenProvider: options.accessTokenProvider,
      ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
      ...(options.onAuthenticationFailure === undefined
        ? {}
        : { onAuthenticationFailure: options.onAuthenticationFailure }),
    });
    this.#stream = new AdminEventStreamClient({
      baseUrl,
      issueAccessToken: () => this.#issueAccessToken(),
      mapEvent: toAdminEvent,
      probeAuthentication: () => this.#probeAuthentication(),
      ...(options.webSocketFactory === undefined
        ? {}
        : { webSocketFactory: options.webSocketFactory }),
      ...(options.reconnectDelayMs === undefined
        ? {}
        : { reconnectDelayMs: options.reconnectDelayMs }),
    });
  }

  subscribe(handlers: AdminEventHandlers): AdminEventSubscription {
    return this.#stream.subscribe(handlers);
  }

  async #issueAccessToken(): Promise<AdminEventStreamAccess> {
    const response = await this.#api.request('/api/v1/admin/events/access-tokens', {
      method: 'POST',
      headers: { accept: 'application/json' },
      cache: 'no-store',
    });
    return AdminEventStreamAccessMapper.fromResponse(await response.json());
  }

  async #probeAuthentication(): Promise<void> {
    try {
      const response = await this.#api.request('/api/v1/sessions', {
        method: 'GET',
        headers: { accept: 'application/json' },
        cache: 'no-store',
      });
      await response.body?.cancel();
    } catch {
      // The stream error remains visible; this request only routes HTTP 401s
      // through the shared authentication-failure handler.
    }
  }
}
