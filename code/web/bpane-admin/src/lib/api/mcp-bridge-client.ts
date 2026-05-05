import {
  expectBoolean,
  expectNumber,
  expectRecord,
  expectString,
  optionalString,
} from './control-wire';

export type McpBridgeHealth = {
  readonly status: string;
  readonly clients: number;
  readonly control_session_id: string | null;
  readonly control_session_state: string | null;
  readonly control_session_backend_delegated: boolean;
  readonly bridge_alignment: string | null;
};

export type McpBridgeControlSession = {
  readonly session_id: string | null;
  readonly cdp_endpoint: string | null;
};

export type McpBridgeClientOptions = {
  readonly controlUrl: string | URL;
  readonly fetchImpl?: typeof fetch;
};

export class McpBridgeClient {
  readonly #controlUrl: URL;
  readonly #fetchImpl: typeof fetch;

  constructor(options: McpBridgeClientOptions) {
    this.#controlUrl = new URL(options.controlUrl);
    this.#fetchImpl = options.fetchImpl ?? fetch;
  }

  async getHealth(): Promise<McpBridgeHealth> {
    const response = await this.#send(this.#healthUrl(), { method: 'GET', cache: 'no-store' });
    return McpBridgeMapper.toHealth(await response.json());
  }

  async setControlSession(sessionId: string): Promise<McpBridgeControlSession> {
    const response = await this.#send(this.#controlUrl, {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ session_id: sessionId }),
    });
    return McpBridgeMapper.toControlSession(await response.json());
  }

  async clearControlSession(): Promise<void> {
    await this.#send(this.#controlUrl, { method: 'DELETE' });
  }

  async #send(url: URL, init: RequestInit): Promise<Response> {
    const response = await this.#fetchImpl(url, init);
    if (!response.ok) {
      throw new Error(`MCP bridge returned HTTP ${response.status}: ${await response.text()}`);
    }
    return response;
  }

  #healthUrl(): URL {
    const healthUrl = new URL(this.#controlUrl);
    healthUrl.pathname = '/health';
    healthUrl.search = '';
    return healthUrl;
  }
}

class McpBridgeMapper {
  static toHealth(payload: unknown): McpBridgeHealth {
    const object = expectRecord(payload, 'mcp bridge health');
    return {
      status: expectString(object.status, 'mcp bridge health status'),
      clients: expectNumber(object.clients, 'mcp bridge health clients'),
      control_session_id: optionalString(object.control_session_id, 'control_session_id') ?? null,
      control_session_state: optionalString(object.control_session_state, 'control_session_state') ?? null,
      control_session_backend_delegated: expectBoolean(
        object.control_session_backend_delegated ?? false,
        'control_session_backend_delegated',
      ),
      bridge_alignment: optionalString(object.bridge_alignment, 'bridge_alignment') ?? null,
    };
  }

  static toControlSession(payload: unknown): McpBridgeControlSession {
    const object = expectRecord(payload, 'mcp bridge control session');
    const session = object.session === null || object.session === undefined
      ? null
      : expectRecord(object.session, 'mcp bridge control session resource');
    return {
      session_id: session ? expectString(session.id, 'mcp bridge control session id') : null,
      cdp_endpoint: optionalString(object.cdp_endpoint, 'mcp bridge cdp_endpoint') ?? null,
    };
  }
}
