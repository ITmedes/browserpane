type GatewaySessionState =
  | "pending"
  | "starting"
  | "ready"
  | "active"
  | "idle"
  | "stopping"
  | "stopped"
  | "failed"
  | "expired";

type GatewaySessionOwnerMode = "collaborative" | "exclusive_browser_owner";

export interface GatewaySessionResource {
  id: string;
  state: GatewaySessionState;
  owner_mode: GatewaySessionOwnerMode;
  connect: {
    gateway_url: string;
    transport_path: string;
    auth_type: string;
    ticket_path?: string | null;
    compatibility_mode: string;
  };
  runtime?: {
    binding: string;
    compatibility_mode: string;
    cdp_endpoint?: string | null;
  } | null;
  integration_context?: Record<string, unknown> | null;
}

interface SessionListResponse {
  sessions: GatewaySessionResource[];
}

export type SessionBootstrapMode = "off" | "reuse_or_create";

export interface SessionControlClientOptions {
  gatewayApiUrl: string;
  getHeaders: (extra?: Record<string, string>) => Promise<Record<string, string>>;
  sessionId?: string;
  bootstrapMode?: SessionBootstrapMode;
  ownerMode?: GatewaySessionOwnerMode;
  displayName?: string;
  integrationContext?: Record<string, unknown>;
}

const RUNTIME_CANDIDATE_STATES = new Set<GatewaySessionState>([
  "pending",
  "starting",
  "ready",
  "active",
  "idle",
]);

function isRuntimeCandidate(state: GatewaySessionState): boolean {
  return RUNTIME_CANDIDATE_STATES.has(state);
}

export class SessionControlClient {
  private readonly gatewayApiUrl: string;
  private readonly getHeaders: (extra?: Record<string, string>) => Promise<Record<string, string>>;
  private readonly sessionId: string;
  private readonly bootstrapMode: SessionBootstrapMode;
  private readonly ownerMode: GatewaySessionOwnerMode;
  private readonly displayName: string;
  private readonly integrationContext: Record<string, unknown>;
  private cachedSession: GatewaySessionResource | null = null;

  constructor(options: SessionControlClientOptions) {
    this.gatewayApiUrl = options.gatewayApiUrl.replace(/\/$/, "");
    this.getHeaders = options.getHeaders;
    this.sessionId = (options.sessionId ?? "").trim();
    this.bootstrapMode = options.bootstrapMode ?? "off";
    this.ownerMode = options.ownerMode ?? "collaborative";
    this.displayName = options.displayName ?? "MCP bridge session";
    this.integrationContext = options.integrationContext ?? {};
  }

  hasExplicitSession(): boolean {
    return this.sessionId.length > 0;
  }

  getCachedSession(): GatewaySessionResource | null {
    return this.cachedSession;
  }

  async resolveManagedSession(): Promise<GatewaySessionResource | null> {
    if (this.cachedSession && isRuntimeCandidate(this.cachedSession.state)) {
      return this.cachedSession;
    }

    if (this.hasExplicitSession()) {
      const session = await this.getSession(this.sessionId);
      if (!session) {
        throw new Error(`configured session ${this.sessionId} was not found for the current principal`);
      }
      this.cachedSession = session;
      return session;
    }

    if (this.bootstrapMode === "off") {
      return null;
    }

    const existing = (await this.listSessions()).find((session) =>
      isRuntimeCandidate(session.state),
    );
    if (existing) {
      this.cachedSession = existing;
      return existing;
    }

    try {
      const created = await this.createSession();
      this.cachedSession = created;
      return created;
    } catch (error) {
      if (
        error instanceof SessionControlClientError &&
        error.status === 409
      ) {
        const retry = (await this.listSessions()).find((session) =>
          isRuntimeCandidate(session.state),
        );
        if (retry) {
          this.cachedSession = retry;
          return retry;
        }
      }
      throw error;
    }
  }

  async listSessions(): Promise<GatewaySessionResource[]> {
    const response = await this.fetchJson<SessionListResponse>("/api/v1/sessions");
    return Array.isArray(response.sessions) ? response.sessions : [];
  }

  async getSession(sessionId: string): Promise<GatewaySessionResource | null> {
    try {
      return await this.fetchJson<GatewaySessionResource>(`/api/v1/sessions/${encodeURIComponent(sessionId)}`);
    } catch (error) {
      if (error instanceof SessionControlClientError && error.status === 404) {
        return null;
      }
      throw error;
    }
  }

  private async createSession(): Promise<GatewaySessionResource> {
    return this.fetchJson<GatewaySessionResource>("/api/v1/sessions", {
      method: "POST",
      body: JSON.stringify({
        display_name: this.displayName,
        owner_mode: this.ownerMode,
        integration_context: this.integrationContext,
      }),
      headers: {
        "Content-Type": "application/json",
      },
    });
  }

  private async fetchJson<T>(
    path: string,
    init: RequestInit = {},
  ): Promise<T> {
    const headers = await this.getHeaders({
      Accept: "application/json",
      ...(init.headers as Record<string, string> | undefined),
    });
    const response = await fetch(`${this.gatewayApiUrl}${path}`, {
      ...init,
      headers,
    });
    if (!response.ok) {
      let message = `${response.status} ${response.statusText}`.trim();
      try {
        const payload = (await response.json()) as { error?: string };
        if (payload?.error) {
          message = payload.error;
        }
      } catch {
        // ignore malformed error bodies
      }
      throw new SessionControlClientError(message, response.status);
    }
    return (await response.json()) as T;
  }
}

class SessionControlClientError extends Error {
  readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "SessionControlClientError";
    this.status = status;
  }
}
