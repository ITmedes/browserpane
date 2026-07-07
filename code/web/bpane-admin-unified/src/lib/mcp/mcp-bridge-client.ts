export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export type AccessTokenProvider = () => Promise<string | null> | string | null;

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
  readonly accessTokenProvider?: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class McpBridgeClient {
  readonly #controlUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider | undefined;
  readonly #fetchImpl: FetchLike;
  readonly #onAuthenticationFailure: (() => void) | undefined;

  constructor(options: McpBridgeClientOptions) {
    this.#controlUrl = new URL(options.controlUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
    this.#fetchImpl = options.fetchImpl ?? fetch;
    this.#onAuthenticationFailure = options.onAuthenticationFailure;
  }

  async getHealth(): Promise<McpBridgeHealth> {
    const response = await this.#send(this.#healthUrl(), { method: 'GET', cache: 'no-store' });
    return toMcpBridgeHealth(await response.json());
  }

  async setControlSession(sessionId: string): Promise<McpBridgeControlSession> {
    const response = await this.#send(this.#controlUrl, {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ session_id: sessionId }),
    });
    return toMcpBridgeControlSession(await response.json());
  }

  async clearControlSession(): Promise<void> {
    await this.#send(this.#controlUrl, { method: 'DELETE' });
  }

  async #send(url: URL, init: RequestInit): Promise<Response> {
    const headers = new Headers(init.headers);
    if (this.#accessTokenProvider) {
      const accessToken = await this.#accessTokenProvider();
      if (!accessToken) {
        throw new Error('No active admin access token is available for MCP bridge control.');
      }
      headers.set('authorization', `Bearer ${accessToken}`);
    }
    const response = await this.#fetchImpl(url, { ...init, headers });
    if (response.status === 401) {
      this.#onAuthenticationFailure?.();
    }
    if (!response.ok) {
      const detail = await response.text().catch(() => '');
      throw new Error(`MCP bridge request failed with HTTP ${response.status}${detail ? `: ${detail}` : ''}`);
    }
    return response;
  }

  #healthUrl(): URL {
    const healthUrl = new URL(this.#controlUrl);
    const controlPath = healthUrl.pathname.endsWith('/')
      ? healthUrl.pathname.slice(0, -1)
      : healthUrl.pathname;
    const lastSeparator = controlPath.lastIndexOf('/');
    healthUrl.pathname = `${controlPath.slice(0, lastSeparator + 1)}health`;
    healthUrl.search = '';
    healthUrl.hash = '';
    return healthUrl;
  }
}

export function toMcpBridgeHealth(payload: unknown): McpBridgeHealth {
  const object = expectRecord(payload, 'mcp bridge health');
  return {
    status: expectString(object.status, 'mcp bridge health status'),
    clients: optionalNumber(object.clients, 'mcp bridge health clients') ?? 0,
    control_session_id: optionalString(object.control_session_id, 'mcp bridge control_session_id') ?? null,
    control_session_state: optionalString(object.control_session_state, 'mcp bridge control_session_state') ?? null,
    control_session_backend_delegated:
      optionalBoolean(object.control_session_backend_delegated, 'mcp bridge control_session_backend_delegated') ?? false,
    bridge_alignment: optionalString(object.bridge_alignment, 'mcp bridge bridge_alignment') ?? null,
    managed_sessions: toManagedSessions(object.managed_sessions),
  };
}

function toMcpBridgeControlSession(payload: unknown): McpBridgeControlSession {
  const object = expectRecord(payload, 'mcp bridge control session');
  const session = object.session === null || object.session === undefined
    ? null
    : expectRecord(object.session, 'mcp bridge control session resource');
  return {
    session_id: session ? expectString(session.id, 'mcp bridge control session id') : null,
    cdp_endpoint: optionalString(object.cdp_endpoint, 'mcp bridge control cdp_endpoint') ?? null,
  };
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
    clients: optionalNumber(object.clients, `mcp bridge managed_sessions[${index}].clients`) ?? 0,
    state: optionalString(object.state, `mcp bridge managed_sessions[${index}].state`) ?? null,
    mode: optionalString(object.mode, `mcp bridge managed_sessions[${index}].mode`) ?? null,
    visible: optionalBoolean(object.visible, `mcp bridge managed_sessions[${index}].visible`) ?? false,
    backend_delegated:
      optionalBoolean(object.backend_delegated, `mcp bridge managed_sessions[${index}].backend_delegated`) ?? false,
    mcp_owner: optionalBoolean(object.mcp_owner, `mcp bridge managed_sessions[${index}].mcp_owner`),
    cdp_endpoint: optionalString(object.cdp_endpoint, `mcp bridge managed_sessions[${index}].cdp_endpoint`) ?? null,
    playwright_cdp_endpoint:
      optionalString(object.playwright_cdp_endpoint, `mcp bridge managed_sessions[${index}].playwright_cdp_endpoint`) ?? null,
    playwright_effective_cdp_endpoint:
      optionalString(
        object.playwright_effective_cdp_endpoint,
        `mcp bridge managed_sessions[${index}].playwright_effective_cdp_endpoint`,
      ) ?? null,
    alignment: optionalString(object.alignment, `mcp bridge managed_sessions[${index}].alignment`) ?? null,
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
}

function optionalString(value: unknown, label: string): string | null {
  if (value === undefined || value === null || value === '') {
    return null;
  }
  return expectString(value, label);
}

function optionalNumber(value: unknown, label: string): number | null {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`${label} must be a finite number`);
  }
  return value;
}

function optionalBoolean(value: unknown, label: string): boolean | null {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value !== 'boolean') {
    throw new Error(`${label} must be a boolean`);
  }
  return value;
}
