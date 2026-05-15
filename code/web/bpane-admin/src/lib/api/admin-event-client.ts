import { AdminEventMapper, type AdminEvent } from './admin-event-mapper';
import {
  sendAuthenticatedRequest,
  type AccessTokenProvider,
  type AuthenticationFailureHandler,
  type FetchLike,
} from './authenticated-api';

export type AdminEventConnectionStatus = 'connecting' | 'open' | 'closed' | 'reconnecting';

export type AdminEventWebSocket = {
  onopen: (() => void) | null;
  onmessage: ((event: { readonly data: unknown }) => void) | null;
  onclose: (() => void) | null;
  onerror: (() => void) | null;
  close: () => void;
};

export type AdminEventWebSocketFactory = (url: string) => AdminEventWebSocket;

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

export type AdminEventSubscription = {
  readonly close: () => void;
};

export class AdminEventClient {
  readonly #baseUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider;
  readonly #onAuthenticationFailure: AuthenticationFailureHandler | undefined;
  readonly #fetchImpl: FetchLike;
  readonly #webSocketFactory: AdminEventWebSocketFactory;
  readonly #reconnectDelayMs: number;

  constructor(options: AdminEventClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
    this.#onAuthenticationFailure = options.onAuthenticationFailure;
    this.#fetchImpl = options.fetchImpl ?? fetch;
    this.#webSocketFactory = options.webSocketFactory ?? defaultWebSocketFactory;
    this.#reconnectDelayMs = options.reconnectDelayMs ?? 1_500;
  }

  subscribe(handlers: AdminEventHandlers): AdminEventSubscription {
    let closed = false;
    let socket: AdminEventWebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

    const emitStatus = (status: AdminEventConnectionStatus): void => {
      handlers.onStatus?.(status);
    };
    const emitError = (error: unknown): void => {
      handlers.onError?.(error instanceof Error ? error : new Error(String(error)));
    };
    const scheduleReconnect = (): void => {
      if (closed || reconnectTimer) {
        return;
      }
      emitStatus('reconnecting');
      reconnectTimer = setTimeout(() => {
        reconnectTimer = null;
        void connect();
      }, this.#reconnectDelayMs);
    };
    const connect = async (): Promise<void> => {
      try {
        emitStatus('connecting');
        const url = buildAdminEventUrl(this.#baseUrl, await this.#accessTokenProvider());
        if (closed) {
          return;
        }
        let opened = false;
        socket = this.#webSocketFactory(url);
        socket.onopen = () => {
          opened = true;
          emitStatus('open');
        };
        socket.onmessage = (event) => handleMessage(event.data, handlers, emitError);
        socket.onerror = () => {
          emitError(new Error('admin event stream websocket error'));
          void this.#probeAuthentication();
        };
        socket.onclose = () => {
          if (!closed && !opened) {
            void this.#probeAuthentication();
          }
          scheduleReconnect();
        };
      } catch (error) {
        emitError(error);
        scheduleReconnect();
      }
    };

    void connect();
    return {
      close: () => {
        closed = true;
        if (reconnectTimer) {
          clearTimeout(reconnectTimer);
        }
        socket?.close();
        emitStatus('closed');
      },
    };
  }

  async #probeAuthentication(): Promise<void> {
    if (!this.#onAuthenticationFailure) {
      return;
    }
    try {
      const response = await sendAuthenticatedRequest({
        baseUrl: this.#baseUrl,
        accessTokenProvider: this.#accessTokenProvider,
        fetchImpl: this.#fetchImpl,
        onAuthenticationFailure: this.#onAuthenticationFailure,
        method: 'GET',
        path: '/api/v1/sessions',
        accept: 'application/json',
      });
      await response.body?.cancel();
    } catch {
      // The stream error remains the user-visible symptom; the probe only exists
      // to route HTTP 401s through the shared auth-failure handler.
    }
  }
}

export function buildAdminEventUrl(baseUrl: string | URL, accessToken: string): string {
  const url = new URL('/api/v1/admin/events', baseUrl);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  url.searchParams.set('access_token', accessToken);
  return url.toString();
}

function handleMessage(
  data: unknown,
  handlers: AdminEventHandlers,
  emitError: (error: unknown) => void,
): void {
  try {
    handlers.onEvent(AdminEventMapper.toEvent(JSON.parse(String(data))));
  } catch (error) {
    emitError(error);
  }
}

function defaultWebSocketFactory(url: string): AdminEventWebSocket {
  return new WebSocket(url) as AdminEventWebSocket;
}
