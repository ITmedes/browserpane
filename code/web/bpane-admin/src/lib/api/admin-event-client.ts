import { AdminEventMapper, type AdminEvent } from './admin-event-mapper';
import type { AccessTokenProvider } from './control-client';

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
  readonly #webSocketFactory: AdminEventWebSocketFactory;
  readonly #reconnectDelayMs: number;

  constructor(options: AdminEventClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
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
        socket = this.#webSocketFactory(url);
        socket.onopen = () => emitStatus('open');
        socket.onmessage = (event) => handleMessage(event.data, handlers, emitError);
        socket.onerror = () => emitError(new Error('admin event stream websocket error'));
        socket.onclose = () => scheduleReconnect();
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
