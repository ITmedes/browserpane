import { ControlClient } from '../api/control-client';
import type { SessionResource } from '../api/control-types';
import { BrowserSessionSdkLoader } from './browser-session-sdk-loader';
import { DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES } from './browser-session-types';
import type {
  BrowserSessionConnectPreferences,
  BrowserSessionSdk,
  LiveBrowserSessionConnection,
} from './browser-session-types';

export type BrowserSessionSdkProvider = {
  readonly load: () => Promise<BrowserSessionSdk>;
};

export type BrowserSessionConnectorOptions = {
  readonly controlClient: ControlClient;
  readonly sdkProvider?: BrowserSessionSdkProvider;
  readonly certHashUrl?: string;
};

export class BrowserSessionConnector {
  readonly #controlClient: ControlClient;
  readonly #sdkProvider: BrowserSessionSdkProvider;
  readonly #certHashUrl: string;

  constructor(options: BrowserSessionConnectorOptions) {
    this.#controlClient = options.controlClient;
    this.#sdkProvider = options.sdkProvider ?? new BrowserSessionSdkLoader();
    this.#certHashUrl = options.certHashUrl ?? '/cert-hash';
  }

  async connect(
    session: SessionResource,
    container: HTMLElement,
    preferences: BrowserSessionConnectPreferences = DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES,
  ): Promise<LiveBrowserSessionConnection> {
    resetSessionContainer(container);
    const access = await this.#controlClient.issueSessionAccessToken(session.id);
    if (access.token_type !== 'session_connect_ticket') {
      throw new Error(`unsupported session access token type ${access.token_type}`);
    }

    const sdk = await this.#sdkProvider.load();
    const gatewayUrl = `${access.connect.gateway_url}${access.connect.transport_path}`;
    let handle: Awaited<ReturnType<BrowserSessionSdk['BpaneSession']['connect']>>;
    try {
      handle = await sdk.BpaneSession.connect({
        container,
        gatewayUrl,
        connectTicket: access.token,
        clientRole: 'interactive',
        certHashUrl: this.#certHashUrl,
        ...preferences,
      });
    } catch (error) {
      throw browserSessionConnectError(error, gatewayUrl);
    }

    return {
      sessionId: session.id,
      gatewayUrl,
      handle,
    };
  }
}

function resetSessionContainer(container: HTMLElement): void {
  container.replaceChildren();
  container.removeAttribute('style');
}

function browserSessionConnectError(error: unknown, gatewayUrl: string): Error {
  const message = error instanceof Error ? error.message : String(error);
  if (!isOpeningHandshakeFailure(message)) {
    return error instanceof Error ? error : new Error(message);
  }

  const enriched = new Error([
    'WebTransport opening handshake failed before the browser stream opened.',
    `Gateway: ${gatewayUrl}.`,
    'Check that the gateway QUIC/WebTransport endpoint is reachable and trusted.',
    'For local compose, start Chromium with --origin-to-force-quic-on=localhost:4433',
    'and trust the dev SPKI from /cert-fingerprint, then reload the admin app.',
  ].join(' '));
  (enriched as Error & { cause?: unknown }).cause = error;
  return enriched;
}

function isOpeningHandshakeFailure(message: string): boolean {
  return /\bopening handshake failed\b/i.test(message);
}
