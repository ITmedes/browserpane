import { ControlClient } from '../api/control-client';
import type { SessionResource } from '../api/control-types';
import { BrowserSessionSdkLoader } from './browser-session-sdk-loader';
import type {
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
  ): Promise<LiveBrowserSessionConnection> {
    const access = await this.#controlClient.issueSessionAccessToken(session.id);
    if (access.token_type !== 'session_connect_ticket') {
      throw new Error(`unsupported session access token type ${access.token_type}`);
    }

    const sdk = await this.#sdkProvider.load();
    const gatewayUrl = `${access.connect.gateway_url}${access.connect.transport_path}`;
    const handle = await sdk.BpaneSession.connect({
      container,
      gatewayUrl,
      connectTicket: access.token,
      clientRole: 'interactive',
      hiDpi: true,
      audio: true,
      camera: true,
      clipboard: true,
      fileTransfer: true,
      certHashUrl: this.#certHashUrl,
    });

    return {
      sessionId: session.id,
      gatewayUrl,
      handle,
    };
  }
}
