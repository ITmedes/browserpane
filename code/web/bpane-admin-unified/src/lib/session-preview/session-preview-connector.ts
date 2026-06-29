import type { SessionCatalogClient } from '$lib/sessions/session-client';
import type { SessionResource } from '$lib/sessions/session-types';
import { BrowserSessionSdkLoader } from './browser-session-sdk-loader';
import {
  DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES,
  type BrowserSessionConnectPreferences,
  type BrowserSessionSdk,
  type LiveBrowserSessionConnection,
} from './browser-session-types';

export type BrowserSessionSdkProvider = {
  readonly load: () => Promise<BrowserSessionSdk>;
};

export type SessionPreviewConnectorOptions = {
  readonly sessionClient: Pick<SessionCatalogClient, 'issueSessionAccessToken'>;
  readonly sdkProvider?: BrowserSessionSdkProvider;
  readonly certHashUrl?: string;
};

export class SessionPreviewConnector {
  readonly #sessionClient: Pick<SessionCatalogClient, 'issueSessionAccessToken'>;
  readonly #sdkProvider: BrowserSessionSdkProvider;
  readonly #certHashUrl: string;

  constructor(options: SessionPreviewConnectorOptions) {
    this.#sessionClient = options.sessionClient;
    this.#sdkProvider = options.sdkProvider ?? new BrowserSessionSdkLoader();
    this.#certHashUrl = options.certHashUrl ?? '/cert-hash';
  }

  async connect(
    session: SessionResource,
    container: HTMLElement,
    preferences: BrowserSessionConnectPreferences = DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES,
  ): Promise<LiveBrowserSessionConnection> {
    resetSessionContainer(container);
    const access = await this.#sessionClient.issueSessionAccessToken(session.id);
    if (access.token_type !== 'session_connect_ticket') {
      throw new Error(`unsupported session access token type ${access.token_type}`);
    }

    const sdk = await this.#sdkProvider.load();
    const gatewayUrl = `${access.connect.gateway_url}${access.connect.transport_path}`;
    try {
      const handle = await sdk.BpaneSession.connect({
        container,
        gatewayUrl,
        connectTicket: access.token,
        clientRole: 'interactive',
        certHashUrl: this.#certHashUrl,
        ...preferences,
      });
      return {
        sessionId: session.id,
        gatewayUrl,
        handle,
      };
    } catch (error) {
      throw browserSessionConnectError(error, gatewayUrl);
    }
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
    'and trust the dev SPKI from /cert-fingerprint, then reload the preview window.',
  ].join(' '));
  (enriched as Error & { cause?: unknown }).cause = error;
  return enriched;
}

function isOpeningHandshakeFailure(message: string): boolean {
  return /\bopening handshake failed\b/i.test(message);
}
