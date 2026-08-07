import { AdminEventStreamAccessMapper, type AdminEventStreamAccess } from './admin-event-stream-access';

export type AdminEventConnectionStatus = 'connecting' | 'open' | 'closed' | 'reconnecting';

export type AdminEventWebSocket = {
  onopen: (() => void) | null;
  onmessage: ((event: { readonly data: unknown }) => void) | null;
  onclose: (() => void) | null;
  onerror: (() => void) | null;
  send: (data: string) => void;
  close: () => void;
};

export type AdminEventWebSocketFactory = (url: string) => AdminEventWebSocket;

export type AdminEventStreamHandlers<Event> = {
  readonly onEvent: (event: Event) => void;
  readonly onStatus?: (status: AdminEventConnectionStatus) => void;
  readonly onError?: (error: Error) => void;
};

export type AdminEventSubscription = {
  readonly close: () => void;
};

export type AdminEventStreamClientOptions<Event> = {
  readonly baseUrl: string | URL;
  readonly issueAccessToken: () => Promise<AdminEventStreamAccess>;
  readonly mapEvent: (payload: unknown) => Event;
  readonly probeAuthentication?: () => Promise<void>;
  readonly webSocketFactory?: AdminEventWebSocketFactory;
  readonly reconnectDelayMs?: number;
};

export class AdminEventStreamClient<Event> {
  readonly #baseUrl: URL;
  readonly #issueAccessToken: () => Promise<AdminEventStreamAccess>;
  readonly #mapEvent: (payload: unknown) => Event;
  readonly #probeAuthentication: (() => Promise<void>) | undefined;
  readonly #webSocketFactory: AdminEventWebSocketFactory;
  readonly #reconnectDelayMs: number;

  constructor(options: AdminEventStreamClientOptions<Event>) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#issueAccessToken = options.issueAccessToken;
    this.#mapEvent = options.mapEvent;
    this.#probeAuthentication = options.probeAuthentication;
    this.#webSocketFactory = options.webSocketFactory ?? defaultWebSocketFactory;
    this.#reconnectDelayMs = options.reconnectDelayMs ?? 1_500;
  }

  subscribe(handlers: AdminEventStreamHandlers<Event>): AdminEventSubscription {
    let closed = false;
    let socket: AdminEventWebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

    const emitStatus = (status: AdminEventConnectionStatus): void => {
      handlers.onStatus?.(status);
    };
    const emitError = (error: unknown): void => {
      handlers.onError?.(error instanceof Error ? error : new Error(String(error)));
    };
    const probeAuthentication = (): void => {
      void this.#probeAuthentication?.().catch(() => undefined);
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
        const access = await this.#issueAccessToken();
        if (closed) {
          return;
        }
        let authenticated = false;
        socket = this.#webSocketFactory(
          AdminEventStreamAccessMapper.toWebSocketUrl(this.#baseUrl, access.endpointPath),
        );
        socket.onopen = () => {
          socket?.send(JSON.stringify({
            message_type: access.authenticationMessageType,
            token: access.token,
          }));
        };
        socket.onmessage = (event) => {
          if (!authenticated) {
            if (isAuthenticationAcknowledgement(event.data, access.authenticatedMessageType)) {
              authenticated = true;
              emitStatus('open');
              return;
            }
            emitError(new Error('admin event stream rejected its authentication handshake'));
            socket?.close();
            return;
          }
          try {
            handlers.onEvent(this.#mapEvent(JSON.parse(String(event.data))));
          } catch (error) {
            emitError(error);
          }
        };
        socket.onerror = () => {
          emitError(new Error('admin event stream websocket error'));
          probeAuthentication();
        };
        socket.onclose = () => {
          if (!closed && !authenticated) {
            probeAuthentication();
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
}

function isAuthenticationAcknowledgement(data: unknown, expectedMessageType: string): boolean {
  try {
    const parsed = JSON.parse(String(data)) as unknown;
    return Boolean(
      parsed
      && typeof parsed === 'object'
      && (parsed as Record<string, unknown>).message_type === expectedMessageType,
    );
  } catch {
    return false;
  }
}

function defaultWebSocketFactory(url: string): AdminEventWebSocket {
  return new WebSocket(url) as AdminEventWebSocket;
}
