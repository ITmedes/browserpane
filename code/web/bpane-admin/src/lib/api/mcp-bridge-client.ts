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
  readonly managed_sessions: readonly McpManagedSessionHealth[];
};

export type McpManagedSessionHealth = {
  readonly kind: string;
  readonly session_id: string;
  readonly clients: number;
  readonly state: string | null;
  readonly mode: string | null;
  readonly visible: boolean;
  readonly backend_delegated: boolean;
  readonly mcp_owner: boolean | null;
  readonly cdp_endpoint: string | null;
  readonly playwright_cdp_endpoint: string | null;
  readonly playwright_effective_cdp_endpoint: string | null;
  readonly alignment: string | null;
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
      managed_sessions: toManagedSessions(object.managed_sessions),
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

function toManagedSessions(value: unknown): readonly McpManagedSessionHealth[] {
  if (value === undefined || value === null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new Error('mcp bridge managed_sessions must be an array');
  }
  return value.map((entry, index) => toManagedSession(entry, index));
}

function toManagedSession(value: unknown, index: number): McpManagedSessionHealth {
  const object = expectRecord(value, `mcp bridge managed_sessions[${index}]`);
  return {
    kind: expectString(object.kind, `mcp bridge managed_sessions[${index}].kind`),
    session_id: expectString(object.session_id, `mcp bridge managed_sessions[${index}].session_id`),
    clients: expectNumber(object.clients, `mcp bridge managed_sessions[${index}].clients`),
    state: optionalString(object.state, `mcp bridge managed_sessions[${index}].state`) ?? null,
    mode: optionalString(object.mode, `mcp bridge managed_sessions[${index}].mode`) ?? null,
    visible: expectBoolean(object.visible ?? false, `mcp bridge managed_sessions[${index}].visible`),
    backend_delegated: expectBoolean(
      object.backend_delegated ?? false,
      `mcp bridge managed_sessions[${index}].backend_delegated`,
    ),
    mcp_owner: optionalBoolean(object.mcp_owner, `mcp bridge managed_sessions[${index}].mcp_owner`),
    cdp_endpoint: optionalString(object.cdp_endpoint, `mcp bridge managed_sessions[${index}].cdp_endpoint`) ?? null,
    playwright_cdp_endpoint: optionalString(
      object.playwright_cdp_endpoint,
      `mcp bridge managed_sessions[${index}].playwright_cdp_endpoint`,
    ) ?? null,
    playwright_effective_cdp_endpoint: optionalString(
      object.playwright_effective_cdp_endpoint,
      `mcp bridge managed_sessions[${index}].playwright_effective_cdp_endpoint`,
    ) ?? null,
    alignment: optionalString(object.alignment, `mcp bridge managed_sessions[${index}].alignment`) ?? null,
  };
}

function optionalBoolean(value: unknown, label: string): boolean | null {
  if (value === undefined || value === null) {
    return null;
  }
  return expectBoolean(value, label);
}
