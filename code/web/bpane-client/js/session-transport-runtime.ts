import { UnsupportedFeatureError } from './shared/errors.js';

export interface SessionTransportRuntimeInput {
  onConnect?: () => void;
  onDisconnect?: (reason: string) => void;
  onError?: (error: Error) => void;
  onStream?: (stream: WebTransportBidirectionalStream) => void | Promise<void>;
  onDatagram?: (datagram: Uint8Array) => void;
  onDatagramReadError?: (error: unknown) => void;
  sendPing?: () => void;
  pingIntervalMs?: number;
  createTransport?: (url: string, options: WebTransportOptions) => WebTransport;
  fetchFn?: typeof fetch;
  atobFn?: (data: string) => string;
  setIntervalFn?: Window['setInterval'];
  clearIntervalFn?: Window['clearInterval'];
}

export interface SessionTransportConnectOptions {
  gatewayUrl: string;
  accessToken?: string;
  token?: string;
  certHashUrl?: string;
}

export class SessionTransportRuntime {
  private readonly onConnect?: () => void;
  private readonly onDisconnect?: (reason: string) => void;
  private readonly onError?: (error: Error) => void;
  private readonly onStream?: (stream: WebTransportBidirectionalStream) => void | Promise<void>;
  private readonly onDatagram?: (datagram: Uint8Array) => void;
  private readonly onDatagramReadError?: (error: unknown) => void;
  private readonly sendPing?: () => void;
  private readonly pingIntervalMs: number;
  private readonly createTransport?: (url: string, options: WebTransportOptions) => WebTransport;
  private readonly fetchFn: typeof fetch;
  private readonly atobFn: (data: string) => string;
  private readonly setIntervalFn: Window['setInterval'];
  private readonly clearIntervalFn: Window['clearInterval'];

  private transport: WebTransport | null = null;
  private connected = false;
  private pingInterval: number | null = null;

  constructor(input: SessionTransportRuntimeInput) {
    this.onConnect = input.onConnect;
    this.onDisconnect = input.onDisconnect;
    this.onError = input.onError;
    this.onStream = input.onStream;
    this.onDatagram = input.onDatagram;
    this.onDatagramReadError = input.onDatagramReadError;
    this.sendPing = input.sendPing;
    this.pingIntervalMs = input.pingIntervalMs ?? 5000;
    this.createTransport = input.createTransport;
    this.fetchFn = input.fetchFn ?? globalThis.fetch.bind(globalThis);
    this.atobFn = input.atobFn ?? globalThis.atob.bind(globalThis);
    this.setIntervalFn = input.setIntervalFn ?? window.setInterval.bind(window);
    this.clearIntervalFn = input.clearIntervalFn ?? window.clearInterval.bind(window);
  }

  async connect(options: SessionTransportConnectOptions): Promise<void> {
    const nonce = `${Date.now()}.${Math.random().toString(36).slice(2)}`;
    const accessToken = options.accessToken ?? options.token;
    if (!accessToken) {
      throw new Error('missing access token');
    }
    const url = `${options.gatewayUrl}?access_token=${encodeURIComponent(accessToken)}&_=${nonce}`;
    const certHash = options.certHashUrl ? await this.fetchCertHash(options.certHashUrl) : null;

    try {
      const transport = this.instantiateTransport(url, certHash);
      this.transport = transport;
      await transport.ready;
      this.connected = true;
      this.onConnect?.();
      transport.closed.then(() => {
        if (!this.connected) {
          return;
        }
        this.connected = false;
        this.stopPingTimer();
        this.onDisconnect?.('transport closed');
      }).catch((error: unknown) => {
        if (!this.connected) {
          return;
        }
        this.connected = false;
        this.stopPingTimer();
        const runtimeError = error instanceof Error ? error : new Error(String(error));
        this.onError?.(runtimeError);
        this.onDisconnect?.('transport error');
      });

      void this.readStreams(transport);
      void this.readDatagrams(transport);
      this.startPingTimer();
    } catch (error) {
      const runtimeError = error instanceof Error ? error : new Error(String(error));
      this.onError?.(runtimeError);
      throw runtimeError;
    }
  }

  disconnect(): void {
    this.connected = false;
    this.stopPingTimer();
    this.transport?.close();
    this.transport = null;
  }

  private instantiateTransport(url: string, certHash: Uint8Array | null): WebTransport {
    const options: WebTransportOptions = {};
    if (certHash) {
      options.serverCertificateHashes = [{
        algorithm: 'sha-256',
        value: new Uint8Array(certHash).buffer,
      }];
    }

    if (this.createTransport) {
      return this.createTransport(url, options);
    }

    const webTransportCtor = (globalThis as typeof globalThis & {
      WebTransport?: typeof WebTransport;
    }).WebTransport;
    if (!webTransportCtor) {
      throw new UnsupportedFeatureError(
        'bpane.transport.webtransport_unavailable',
        'WebTransport is unavailable in this browser',
      );
    }
    return new webTransportCtor(url, options);
  }

  private async fetchCertHash(url: string): Promise<Uint8Array | null> {
    try {
      const response = await this.fetchFn(url);
      if (!response.ok) {
        return null;
      }
      const base64 = (await response.text()).trim();
      if (base64.length < 10) {
        return null;
      }
      const raw = this.atobFn(base64);
      const bytes = new Uint8Array(raw.length);
      for (let i = 0; i < raw.length; i++) {
        bytes[i] = raw.charCodeAt(i);
      }
      return bytes;
    } catch {
      return null;
    }
  }

  private async readStreams(transport: WebTransport): Promise<void> {
    const reader = transport.incomingBidirectionalStreams.getReader();
    try {
      while (this.connected) {
        const { value, done } = await reader.read();
        if (done) {
          break;
        }
        if (value) {
          void this.onStream?.(value);
        }
      }
    } catch (error) {
      if (!this.connected) {
        return;
      }
      const runtimeError = error instanceof Error ? error : new Error(String(error));
      this.onError?.(runtimeError);
    }
  }

  private async readDatagrams(transport: WebTransport): Promise<void> {
    const reader = transport.datagrams.readable.getReader();
    try {
      while (this.connected) {
        const { value, done } = await reader.read();
        if (done) {
          break;
        }
        if (value) {
          this.onDatagram?.(this.toUint8Array(value));
        }
      }
    } catch (error) {
      if (!this.connected) {
        return;
      }
      this.onDatagramReadError?.(error);
    }
  }

  private startPingTimer(): void {
    this.stopPingTimer();
    this.pingInterval = this.setIntervalFn(() => {
      if (!this.connected) {
        return;
      }
      this.sendPing?.();
    }, this.pingIntervalMs);
  }

  private stopPingTimer(): void {
    if (this.pingInterval === null) {
      return;
    }
    this.clearIntervalFn(this.pingInterval);
    this.pingInterval = null;
  }

  private toUint8Array(value: ArrayBufferLike | ArrayBufferView): Uint8Array {
    if (value instanceof Uint8Array) {
      return value;
    }
    if (ArrayBuffer.isView(value)) {
      return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
    }
    return new Uint8Array(value);
  }
}
