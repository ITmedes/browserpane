import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { SSEServerTransport } from "@modelcontextprotocol/sdk/server/sse.js";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import {
  CallToolRequestSchema,
  isInitializeRequest,
  ListToolsRequestSchema,
  ListResourcesRequestSchema,
  ListPromptsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { randomUUID } from "node:crypto";
import { lookup } from "node:dns/promises";
import http from "node:http";
import { isIP } from "node:net";
import {
  buildManagedSessionHealth,
  type BridgeHealthAlignment,
} from "./mcp-health.js";
import {
  GatewaySessionAutomationAccessResponse,
  GatewaySessionResource,
  SessionControlClient,
} from "./session-control-client.js";
import {
  isStreamableMcpPath,
  isSsePath,
  selectedBrowserPaneSessionId,
} from "./session-selector.js";
import { SupervisorMonitor } from "./supervisor-monitor.js";

// ── Configuration ────────────────────────────────────────────────────

const GATEWAY_API_URL = process.env.BPANE_GATEWAY_API_URL ?? "http://localhost:8932";
const MCP_PORT = parseInt(process.env.BPANE_MCP_PORT ?? "8931", 10);
const MCP_RESOLUTION = process.env.BPANE_MCP_RESOLUTION ?? "1600x900";
const FALLBACK_CDP_ENDPOINT = (process.env.BPANE_CDP_ENDPOINT ?? "").trim();
const SUPERVISED_DELAY_MS = parseInt(process.env.BPANE_MCP_SUPERVISED_DELAY_MS ?? "1500", 10);
const POLL_INTERVAL_MS = parseInt(process.env.BPANE_MCP_POLL_INTERVAL_MS ?? "2000", 10);
const GATEWAY_OIDC_TOKEN_URL = process.env.BPANE_GATEWAY_OIDC_TOKEN_URL ?? "";
const GATEWAY_OIDC_CLIENT_ID = process.env.BPANE_GATEWAY_OIDC_CLIENT_ID ?? "";
const GATEWAY_OIDC_CLIENT_SECRET = process.env.BPANE_GATEWAY_OIDC_CLIENT_SECRET ?? "";
const GATEWAY_OIDC_SCOPES = process.env.BPANE_GATEWAY_OIDC_SCOPES ?? "";
const SESSION_BOOTSTRAP_MODE = (
  process.env.BPANE_SESSION_BOOTSTRAP_MODE ?? "off"
).trim().toLowerCase();
const SESSION_ID = (process.env.BPANE_SESSION_ID ?? "").trim();
const DEFAULT_RUNTIME_TARGET_KEY = "__default__";

// ── Helpers ──────────────────────────────────────────────────────────

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function parseResolution(s: string): { width: number; height: number } {
  const [w, h] = s.split("x").map(Number);
  if (!w || !h) throw new Error(`Invalid resolution: ${s}`);
  return { width: w, height: h };
}

async function readJsonBody<T>(req: http.IncomingMessage): Promise<T> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  const raw = Buffer.concat(chunks).toString("utf8").trim();
  if (!raw) {
    throw new Error("request body is required");
  }
  return JSON.parse(raw) as T;
}

function singleHeader(value: string | string[] | undefined): string | undefined {
  return Array.isArray(value) ? value[0] : value;
}

function writeJsonRpcError(
  res: http.ServerResponse,
  statusCode: number,
  message: string,
  code = -32000,
): void {
  if (res.headersSent) {
    return;
  }
  res.writeHead(statusCode, { "Content-Type": "application/json" });
  res.end(
    JSON.stringify({
      jsonrpc: "2.0",
      error: {
        code,
        message,
      },
      id: null,
    }),
  );
}

class GatewayTokenManager {
  private accessToken: string | null = null;
  private expiresAtMs = 0;

  isEnabled(): boolean {
    return Boolean(GATEWAY_OIDC_TOKEN_URL && GATEWAY_OIDC_CLIENT_ID && GATEWAY_OIDC_CLIENT_SECRET);
  }

  async getAuthHeaders(extra: Record<string, string> = {}): Promise<Record<string, string>> {
    if (!this.isEnabled()) {
      return extra;
    }
    const token = await this.getAccessToken();
    return {
      ...extra,
      Authorization: `Bearer ${token}`,
    };
  }

  private async getAccessToken(): Promise<string> {
    const now = Date.now();
    if (this.accessToken && now < this.expiresAtMs - 30_000) {
      return this.accessToken;
    }

    const body = new URLSearchParams({
      grant_type: "client_credentials",
      client_id: GATEWAY_OIDC_CLIENT_ID,
      client_secret: GATEWAY_OIDC_CLIENT_SECRET,
    });
    if (GATEWAY_OIDC_SCOPES.trim()) {
      body.set("scope", GATEWAY_OIDC_SCOPES.trim());
    }

    const response = await fetch(GATEWAY_OIDC_TOKEN_URL, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body,
    });
    if (!response.ok) {
      throw new Error(`token endpoint returned ${response.status}`);
    }

    const payload = (await response.json()) as {
      access_token?: string;
      expires_in?: number;
    };
    if (!payload.access_token) {
      throw new Error("token endpoint returned no access_token");
    }

    this.accessToken = payload.access_token;
    this.expiresAtMs = now + Math.max(30, payload.expires_in ?? 60) * 1000;
    return this.accessToken;
  }
}

const gatewayTokenManager = new GatewayTokenManager();

class SessionAutomationAccessManager {
  private cachedAccessBySessionId = new Map<string, GatewaySessionAutomationAccessResponse>();

  constructor(private sessionControlClient: SessionControlClient) {}

  clear(sessionId?: string | null): void {
    if (!sessionId) {
      this.cachedAccessBySessionId.clear();
      return;
    }
    this.cachedAccessBySessionId.delete(sessionId);
  }

  getCached(session: GatewaySessionResource | null): GatewaySessionAutomationAccessResponse | null {
    if (!session) {
      return null;
    }
    return this.cachedAccessBySessionId.get(session.id) ?? null;
  }

  async get(session: GatewaySessionResource | null): Promise<GatewaySessionAutomationAccessResponse | null> {
    if (!session) {
      return null;
    }
    const now = Date.now();
    const cachedAccess = this.cachedAccessBySessionId.get(session.id);
    if (cachedAccess) {
      const expiresAtMs = Date.parse(cachedAccess.expires_at);
      if (Number.isFinite(expiresAtMs) && now < expiresAtMs - 30_000) {
        return cachedAccess;
      }
    }
    const issued = await this.sessionControlClient.issueAutomationAccess(session.id);
    this.cachedAccessBySessionId.set(session.id, issued);
    return issued;
  }
}

// ── Register MCP owner with gateway ──────────────────────────────────

function sessionStatusPath(session: GatewaySessionResource | null): string {
  return session
    ? `/api/v1/sessions/${encodeURIComponent(session.id)}/status`
    : "/api/session/status";
}

function sessionMcpOwnerPath(session: GatewaySessionResource | null): string {
  return session
    ? `/api/v1/sessions/${encodeURIComponent(session.id)}/mcp-owner`
    : "/api/session/mcp-owner";
}

function automationStatusPath(
  session: GatewaySessionResource | null,
  access: GatewaySessionAutomationAccessResponse | null,
): string {
  return access?.automation.status_path ?? sessionStatusPath(session);
}

function automationMcpOwnerPath(
  session: GatewaySessionResource | null,
  access: GatewaySessionAutomationAccessResponse | null,
): string {
  return access?.automation.mcp_owner_path ?? sessionMcpOwnerPath(session);
}

function runtimeCdpEndpoint(session: GatewaySessionResource | null): string | null {
  const runtimeEndpoint = session?.runtime?.cdp_endpoint?.trim();
  if (runtimeEndpoint) {
    return runtimeEndpoint;
  }

  const integrationContextEndpoint = session?.integration_context?.cdp_endpoint;
  if (typeof integrationContextEndpoint === "string" && integrationContextEndpoint.trim()) {
    return integrationContextEndpoint.trim();
  }

  return null;
}

function resolveManagedCdpEndpoint(
  session: GatewaySessionResource | null,
  access: GatewaySessionAutomationAccessResponse | null,
): string {
  const automationEndpoint = access?.automation.endpoint_url?.trim();
  if (automationEndpoint) {
    return automationEndpoint;
  }
  const runtimeEndpoint = runtimeCdpEndpoint(session);
  if (runtimeEndpoint) {
    return runtimeEndpoint;
  }
  if (FALLBACK_CDP_ENDPOINT) {
    return FALLBACK_CDP_ENDPOINT;
  }
  if (session) {
    throw new Error(
      `session ${session.id} does not expose a runtime cdp_endpoint and BPANE_CDP_ENDPOINT is not configured`,
    );
  }
  throw new Error(
    "no managed session is selected and BPANE_CDP_ENDPOINT is not configured",
  );
}

function describeManagedCdpEndpoint(
  session: GatewaySessionResource | null,
  access: GatewaySessionAutomationAccessResponse | null = null,
): string | null {
  try {
    return resolveManagedCdpEndpoint(session, access);
  } catch {
    return null;
  }
}

function isSessionDelegatedToBridge(session: GatewaySessionResource | null): boolean {
  if (!session?.automation_delegate) {
    return false;
  }
  if (!GATEWAY_OIDC_CLIENT_ID) {
    return true;
  }
  return session.automation_delegate.client_id === GATEWAY_OIDC_CLIENT_ID;
}

function deriveBridgeHealthAlignment(
  selectedSession: GatewaySessionResource | null,
  visibleSession: GatewaySessionResource | null,
  expectedCdpEndpoint: string | null,
  playwrightCdpEndpoint: string | null,
): BridgeHealthAlignment {
  if (!selectedSession) {
    return "unmanaged";
  }
  if (!visibleSession) {
    return "control_session_not_visible";
  }
  if (!isSessionDelegatedToBridge(visibleSession)) {
    return "control_session_not_delegated";
  }
  if (
    expectedCdpEndpoint
    && playwrightCdpEndpoint
    && expectedCdpEndpoint !== playwrightCdpEndpoint
  ) {
    return "playwright_endpoint_mismatch";
  }
  return "aligned";
}

function isChromiumDevToolsSafeHostname(hostname: string): boolean {
  return (
    hostname === "localhost"
    || hostname === "127.0.0.1"
    || hostname === "::1"
    || isIP(hostname) !== 0
  );
}

async function resolveChromiumDevToolsEndpoint(cdpEndpoint: string): Promise<string> {
  const url = new URL(cdpEndpoint);
  if (isChromiumDevToolsSafeHostname(url.hostname)) {
    return cdpEndpoint;
  }

  const resolved = await lookup(url.hostname, { family: 4 });
  url.hostname = resolved.address;
  return url.toString();
}

async function gatewaySessionHeaders(
  session: GatewaySessionResource | null,
  automationAccessManager: SessionAutomationAccessManager,
  extra: Record<string, string> = {},
): Promise<Record<string, string>> {
  const access = await automationAccessManager.get(session);
  if (access) {
    return {
      ...extra,
      [access.automation.auth_header]: access.token,
    };
  }
  return gatewayTokenManager.getAuthHeaders(extra);
}

async function registerMcpOwner(
  session: GatewaySessionResource | null,
  automationAccessManager: SessionAutomationAccessManager,
  width: number,
  height: number,
): Promise<void> {
  const maxRetries = 30;
  for (let i = 0; i < maxRetries; i++) {
    try {
      const access = await automationAccessManager.get(session);
      const headers = await gatewaySessionHeaders(session, automationAccessManager, {
        "Content-Type": "application/json",
      });
      const resp = await fetch(
        `${GATEWAY_API_URL}${automationMcpOwnerPath(session, access)}`,
        {
          method: "POST",
          headers,
          body: JSON.stringify({ width, height }),
        },
      );
      if (resp.ok) {
        const sessionSuffix = session ? ` for session ${session.id}` : "";
        console.log(
          `[mcp-bridge] registered as MCP owner${sessionSuffix} at ${width}x${height}`,
        );
        return;
      }
      if (!session && resp.status === 404) {
        console.warn(
          "[mcp-bridge] legacy MCP owner route is unavailable; continuing with fallback CDP endpoint",
        );
        return;
      }
      console.warn(`[mcp-bridge] gateway returned ${resp.status}, retrying...`);
    } catch {
      console.warn(`[mcp-bridge] gateway not available, retrying in 2s... (${i + 1}/${maxRetries})`);
    }
    await sleep(2000);
  }
  throw new Error("Failed to register MCP owner with gateway after retries");
}

async function unregisterMcpOwner(
  session: GatewaySessionResource | null,
  automationAccessManager: SessionAutomationAccessManager,
): Promise<void> {
  try {
    const headers = await gatewaySessionHeaders(session, automationAccessManager);
    const access = await automationAccessManager.get(session);
    await fetch(`${GATEWAY_API_URL}${automationMcpOwnerPath(session, access)}`, {
      method: "DELETE",
      headers,
    });
    if (session) {
      console.log(`[mcp-bridge] unregistered MCP owner for session ${session.id}`);
    } else {
      console.log("[mcp-bridge] unregistered MCP owner");
    }
  } catch {
    console.warn("[mcp-bridge] failed to unregister MCP owner (gateway unavailable)");
  }
}

// ── Spawn @playwright/mcp subprocess (STDIO mode) ───────────────────

function spawnPlaywrightMcp(cdpEndpoint: string): StdioClientTransport {
  return new StdioClientTransport({
    command: "npx",
    args: ["@playwright/mcp@latest", "--cdp-endpoint", cdpEndpoint],
  });
}

class PlaywrightRuntimeController {
  private client: Client | null = null;
  private requestedCdpEndpoint: string | null = null;
  private effectiveCdpEndpoint: string | null = null;

  async ensureConnected(
    session: GatewaySessionResource | null,
    automationAccessManager: SessionAutomationAccessManager,
  ): Promise<Client> {
    const access = await automationAccessManager.get(session);
    const nextEndpoint = resolveManagedCdpEndpoint(session, access);
    if (this.client && this.requestedCdpEndpoint === nextEndpoint) {
      return this.client;
    }

    await this.close();

    const effectiveEndpoint = await resolveChromiumDevToolsEndpoint(nextEndpoint);
    const transport = spawnPlaywrightMcp(effectiveEndpoint);
    const client = new Client(
      { name: "bpane-mcp-bridge", version: "0.1.0" },
      { capabilities: {} },
    );
    await client.connect(transport);
    this.client = client;
    this.requestedCdpEndpoint = nextEndpoint;
    this.effectiveCdpEndpoint = effectiveEndpoint;

    const sessionSuffix = session ? ` for session ${session.id}` : "";
    const endpointSuffix = effectiveEndpoint === nextEndpoint
      ? nextEndpoint
      : `${nextEndpoint} (resolved to ${effectiveEndpoint})`;
    console.log(
      `[mcp-bridge] connected to @playwright/mcp subprocess${sessionSuffix} via ${endpointSuffix}`,
    );

    return client;
  }

  async close(): Promise<void> {
    if (!this.client) {
      this.requestedCdpEndpoint = null;
      this.effectiveCdpEndpoint = null;
      return;
    }

    const client = this.client;
    const previousEndpoint = this.requestedCdpEndpoint;
    this.client = null;
    this.requestedCdpEndpoint = null;
    this.effectiveCdpEndpoint = null;

    try {
      await client.close();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.warn(`[mcp-bridge] failed to close @playwright/mcp subprocess cleanly: ${message}`);
    }

    if (previousEndpoint) {
      console.log(`[mcp-bridge] closed @playwright/mcp subprocess for ${previousEndpoint}`);
    }
  }

  getCurrentEndpoint(): string | null {
    return this.requestedCdpEndpoint;
  }

  getEffectiveEndpoint(): string | null {
    return this.effectiveCdpEndpoint;
  }
}

type RuntimeTarget = {
  key: string;
  session: GatewaySessionResource | null;
  runtime: PlaywrightRuntimeController;
  monitor: SupervisorMonitor;
  explicit: boolean;
};

function runtimeTargetKey(session: GatewaySessionResource | null): string {
  return session?.id ?? DEFAULT_RUNTIME_TARGET_KEY;
}

// ── Per-connection MCP Server factory ────────────────────────────────

function createServerForConnection(
  playwrightClient: Client,
  monitor: SupervisorMonitor,
): Server {
  const server = new Server(
    { name: "bpane-mcp-bridge", version: "0.1.0" },
    {
      capabilities: {
        tools: {},
        resources: {},
        prompts: {},
      },
    },
  );

  // Proxy tools/list
  server.setRequestHandler(ListToolsRequestSchema, async () => {
    const result = await playwrightClient.listTools();
    return { tools: result.tools };
  });

  // Proxy tools/call with supervisor-aware delay
  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const supervisorCount = monitor.getBrowserClientCount();

    // Add delay BEFORE executing the action so supervisors can see each step
    if (supervisorCount > 0) {
      console.log(
        `[mcp-bridge] ${supervisorCount} supervisor(s) watching — delaying ${SUPERVISED_DELAY_MS}ms before: ${request.params.name}`,
      );
      await sleep(SUPERVISED_DELAY_MS);
    }

    const result = await playwrightClient.callTool({
      name: request.params.name,
      arguments: request.params.arguments,
    });

    return { content: result.content as any };
  });

  // Proxy resources/list (if supported)
  server.setRequestHandler(ListResourcesRequestSchema, async () => {
    try {
      const result = await playwrightClient.listResources();
      return { resources: result.resources };
    } catch {
      return { resources: [] };
    }
  });

  // Proxy prompts/list (if supported)
  server.setRequestHandler(ListPromptsRequestSchema, async () => {
    try {
      const result = await playwrightClient.listPrompts();
      return { prompts: result.prompts };
    } catch {
      return { prompts: [] };
    }
  });

  return server;
}

// ── Main ─────────────────────────────────────────────────────────────

async function main() {
  const { width, height } = parseResolution(MCP_RESOLUTION);
  const managedSessionLocked = SESSION_ID.length > 0;
  const sessionControlClient = new SessionControlClient({
    gatewayApiUrl: GATEWAY_API_URL,
    getHeaders: (extra) => gatewayTokenManager.getAuthHeaders(extra ?? {}),
    sessionId: SESSION_ID,
    bootstrapMode: SESSION_BOOTSTRAP_MODE === "reuse_or_create" ? "reuse_or_create" : "off",
    ownerMode: "collaborative",
    displayName: "MCP bridge session",
    integrationContext: {
      source: "mcp-bridge",
    },
  });
  const automationAccessManager = new SessionAutomationAccessManager(sessionControlClient);
  let managedSession = await sessionControlClient.resolveManagedSession();
  const playwrightRuntime = new PlaywrightRuntimeController();

  // 1. Do NOT register as MCP owner at startup — wait for first SSE client.
  //    This avoids locking the resolution when no MCP client is connected.

  // 2. Start supervisor monitor
  const monitor = new SupervisorMonitor(
    GATEWAY_API_URL,
    POLL_INTERVAL_MS,
    () => gatewaySessionHeaders(managedSession, automationAccessManager),
    automationStatusPath(managedSession, automationAccessManager.getCached(managedSession)),
  );
  monitor.start();
  const explicitRuntimeTargets = new Map<string, RuntimeTarget>();
  const mcpSessionTargetKeys = new Map<string, string>();

  function setManagedSession(next: GatewaySessionResource | null): void {
    const previousSessionId = managedSession?.id ?? null;
    managedSession = next;
    automationAccessManager.clear(previousSessionId);
    monitor.setStatusPath(
      automationStatusPath(managedSession, automationAccessManager.getCached(managedSession)),
    );
  }

  function defaultRuntimeTarget(): RuntimeTarget {
    return {
      key: runtimeTargetKey(managedSession),
      session: managedSession,
      runtime: playwrightRuntime,
      monitor,
      explicit: false,
    };
  }

  async function explicitRuntimeTarget(sessionId: string): Promise<RuntimeTarget> {
    const resolved = await sessionControlClient.getSession(sessionId);
    if (!resolved) {
      throw new Error(`BrowserPane session ${sessionId} is not visible to the MCP bridge`);
    }
    if (!isSessionDelegatedToBridge(resolved)) {
      throw new Error(`BrowserPane session ${sessionId} is not delegated to the MCP bridge`);
    }
    if (managedSession?.id === resolved.id) {
      return defaultRuntimeTarget();
    }

    const existing = explicitRuntimeTargets.get(resolved.id);
    if (existing) {
      return existing;
    }

    const targetMonitor = new SupervisorMonitor(
      GATEWAY_API_URL,
      POLL_INTERVAL_MS,
      () => gatewaySessionHeaders(resolved, automationAccessManager),
      sessionStatusPath(resolved),
    );
    targetMonitor.start();
    const target = {
      key: runtimeTargetKey(resolved),
      session: resolved,
      runtime: new PlaywrightRuntimeController(),
      monitor: targetMonitor,
      explicit: true,
    };
    explicitRuntimeTargets.set(resolved.id, target);
    return target;
  }

  async function runtimeTargetForRequest(
    req: http.IncomingMessage,
    url: URL,
  ): Promise<RuntimeTarget> {
    const selectedSessionId = selectedBrowserPaneSessionId(req, url);
    return selectedSessionId
      ? await explicitRuntimeTarget(selectedSessionId)
      : defaultRuntimeTarget();
  }

  function clientCountForTarget(targetKey: string): number {
    let count = 0;
    for (const value of mcpSessionTargetKeys.values()) {
      if (value === targetKey) {
        count += 1;
      }
    }
    return count;
  }

  async function releaseOwnershipIfIdle(target: RuntimeTarget): Promise<void> {
    if (clientCountForTarget(target.key) > 0) {
      return;
    }
    console.log(`[mcp-bridge] no MCP clients remaining for target ${target.key} — clearing ownership`);
    await unregisterMcpOwner(target.session, automationAccessManager);
    await target.runtime.close();
    if (target.explicit && target.session) {
      target.monitor.stop();
      explicitRuntimeTargets.delete(target.session.id);
    }
  }

  async function closeAllRuntimeTargets(): Promise<void> {
    await unregisterMcpOwner(managedSession, automationAccessManager);
    await playwrightRuntime.close();
    for (const target of explicitRuntimeTargets.values()) {
      await unregisterMcpOwner(target.session, automationAccessManager);
      await target.runtime.close();
      target.monitor.stop();
    }
    explicitRuntimeTargets.clear();
  }

  async function managedSessionHealth(
    kind: "control" | "selected",
    session: GatewaySessionResource | null,
    runtime: PlaywrightRuntimeController,
    clients: number,
  ) {
    let visibleSession: GatewaySessionResource | null = null;
    if (session) {
      try {
        visibleSession = await sessionControlClient.getSession(session.id);
      } catch {
        visibleSession = null;
      }
    }
    const cdpEndpoint = describeManagedCdpEndpoint(
      visibleSession ?? session,
      automationAccessManager.getCached(session),
    );
    const playwrightCdpEndpoint = runtime.getCurrentEndpoint();
    return {
      visibleSession,
      cdpEndpoint,
      entry: buildManagedSessionHealth({
        kind,
        session,
        visibleSession,
        clients,
        backendDelegated: isSessionDelegatedToBridge(visibleSession),
        mcpOwner: await fetchMcpOwner(visibleSession ?? session),
        cdpEndpoint,
        playwrightCdpEndpoint,
        playwrightEffectiveCdpEndpoint: runtime.getEffectiveEndpoint(),
        alignment: deriveBridgeHealthAlignment(
          session,
          visibleSession,
          cdpEndpoint,
          playwrightCdpEndpoint,
        ),
      }),
    };
  }

  async function fetchMcpOwner(session: GatewaySessionResource | null): Promise<boolean | null> {
    if (!session) {
      return null;
    }
    try {
      const access = await automationAccessManager.get(session);
      const headers = await gatewaySessionHeaders(session, automationAccessManager);
      const response = await fetch(`${GATEWAY_API_URL}${automationStatusPath(session, access)}`, {
        headers,
      });
      if (!response.ok) {
        return null;
      }
      const payload = (await response.json()) as { mcp_owner?: unknown };
      return typeof payload.mcp_owner === "boolean" ? payload.mcp_owner : null;
    } catch {
      return null;
    }
  }

  if (managedSession) {
    console.log(
      `[mcp-bridge] resolved control session ${managedSession.id} (${managedSession.state}, ${managedSession.connect.compatibility_mode})`,
    );
    const cdpEndpoint = describeManagedCdpEndpoint(
      managedSession,
      automationAccessManager.getCached(managedSession),
    );
    if (cdpEndpoint) {
      console.log(`[mcp-bridge] control session runtime endpoint: ${cdpEndpoint}`);
    }
  } else {
    console.log(
      "[mcp-bridge] running without a managed control session; legacy runtime ownership endpoints remain in use",
    );
    if (FALLBACK_CDP_ENDPOINT) {
      console.log(`[mcp-bridge] fallback CDP endpoint: ${FALLBACK_CDP_ENDPOINT}`);
    }
  }

  // 4. Start HTTP/SSE server for external MCP clients.
  //    Each SSE connection gets its own Server instance to avoid the
  //    "Already connected to a transport" error.
  const transports = new Map<string, SSEServerTransport>();
  const streamableTransports = new Map<string, StreamableHTTPServerTransport>();
  const servers = new Map<string, Server>();

  async function handleStreamableHttpRequest(
    req: http.IncomingMessage,
    res: http.ServerResponse,
    url: URL,
  ): Promise<void> {
    let parsedBody: unknown | undefined;
    if (req.method === "POST") {
      try {
        parsedBody = await readJsonBody<unknown>(req);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        writeJsonRpcError(res, 400, `Parse error: ${message}`, -32700);
        return;
      }
    }

    const requestedSessionId = singleHeader(req.headers["mcp-session-id"]);
    if (requestedSessionId) {
      const transport = streamableTransports.get(requestedSessionId);
      if (!transport) {
        writeJsonRpcError(res, 404, "Session not found", -32001);
        return;
      }
      try {
        const selectedSessionId = selectedBrowserPaneSessionId(req, url);
        const targetKey = mcpSessionTargetKeys.get(requestedSessionId);
        if (selectedSessionId && targetKey && targetKey !== selectedSessionId) {
          writeJsonRpcError(res, 409, "MCP session is bound to a different BrowserPane session", -32005);
          return;
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        writeJsonRpcError(res, 400, message, -32004);
        return;
      }
      await transport.handleRequest(req, res, parsedBody);
      return;
    }

    if (req.method !== "POST" || !isInitializeRequest(parsedBody)) {
      writeJsonRpcError(res, 400, "Bad Request: No valid session ID provided");
      return;
    }

    let target: RuntimeTarget;
    try {
      target = await runtimeTargetForRequest(req, url);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      writeJsonRpcError(res, 404, message, -32004);
      return;
    }

    const needsBridgeBootstrap = clientCountForTarget(target.key) === 0;
    let sessionId: string | null = null;
    let server: Server | null = null;
    let transport: StreamableHTTPServerTransport | null = null;
    try {
      if (needsBridgeBootstrap) {
        await registerMcpOwner(target.session, automationAccessManager, width, height);
      }
      const playwrightClient = await target.runtime.ensureConnected(
        target.session,
        automationAccessManager,
      );

      transport = new StreamableHTTPServerTransport({
        sessionIdGenerator: () => randomUUID(),
        onsessioninitialized: (initializedSessionId) => {
          sessionId = initializedSessionId;
          if (transport && server) {
            streamableTransports.set(initializedSessionId, transport);
            servers.set(initializedSessionId, server);
            mcpSessionTargetKeys.set(initializedSessionId, target.key);
          }
          console.log(
            `[mcp-bridge] MCP client connected (session=${initializedSessionId}, bpane_target=${target.key}, transport=streamable_http, total=${servers.size}, cdp=${target.runtime.getCurrentEndpoint() ?? "unknown"})`,
          );
        },
      });
      server = createServerForConnection(playwrightClient, target.monitor);
      server.onclose = () => {
        const closedSessionId = sessionId ?? transport?.sessionId;
        if (closedSessionId) {
          streamableTransports.delete(closedSessionId);
          servers.delete(closedSessionId);
          mcpSessionTargetKeys.delete(closedSessionId);
          console.log(
            `[mcp-bridge] MCP client disconnected (session=${closedSessionId}, transport=streamable_http, remaining=${servers.size})`,
          );
          void releaseOwnershipIfIdle(target);
        }
      };

      await server.connect(transport);
      await transport.handleRequest(req, res, parsedBody);
    } catch (error) {
      if (sessionId) {
        streamableTransports.delete(sessionId);
        servers.delete(sessionId);
        mcpSessionTargetKeys.delete(sessionId);
      }
      try {
        await server?.close();
      } catch {
        // ignore cleanup failures after a failed streamable HTTP request
      }
      if (needsBridgeBootstrap) {
        await releaseOwnershipIfIdle(target);
      }
      const message = error instanceof Error ? error.message : String(error);
      writeJsonRpcError(res, 503, message, -32603);
    }
  }

  const httpServer = http.createServer(async (req, res) => {
    // CORS headers
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS");
    res.setHeader(
      "Access-Control-Allow-Headers",
      "Accept, Content-Type, Authorization, Mcp-Protocol-Version, Mcp-Session-Id, Bpane-Session-Id",
    );

    if (req.method === "OPTIONS") {
      res.writeHead(204);
      res.end();
      return;
    }

    const url = new URL(req.url ?? "/", `http://localhost:${MCP_PORT}`);

    if (url.pathname === "/health") {
      const controlHealth = await managedSessionHealth(
        "control",
        managedSession,
        playwrightRuntime,
        managedSession ? clientCountForTarget(runtimeTargetKey(managedSession)) : 0,
      );
      const selectedHealth = await Promise.all(
        Array.from(explicitRuntimeTargets.values()).map((target) =>
          managedSessionHealth("selected", target.session, target.runtime, clientCountForTarget(target.key)),
        ),
      );
      const managedSessions = [
        ...(managedSession ? [controlHealth.entry] : []),
        ...selectedHealth.map((health) => health.entry),
      ];
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          status: "ok",
          clients: servers.size,
          supervisors: monitor.getBrowserClientCount(),
          resolution: { width, height },
          supervised_delay_ms: SUPERVISED_DELAY_MS,
          control_session_id: managedSession?.id ?? null,
          control_session_state:
            controlHealth.visibleSession?.state ?? managedSession?.state ?? null,
          control_session_mode:
            controlHealth.visibleSession?.connect.compatibility_mode
            ?? managedSession?.connect.compatibility_mode
            ?? null,
          control_session_visible: Boolean(controlHealth.visibleSession),
          control_session_backend_delegated:
            isSessionDelegatedToBridge(controlHealth.visibleSession),
          bridge_alignment: controlHealth.entry.alignment,
          control_session_cdp_endpoint: controlHealth.cdpEndpoint,
          playwright_cdp_endpoint: playwrightRuntime.getCurrentEndpoint(),
          playwright_effective_cdp_endpoint: playwrightRuntime.getEffectiveEndpoint(),
          managed_sessions: managedSessions,
          selected_session_clients: Array.from(explicitRuntimeTargets.values()).map((target) => ({
            session_id: target.session?.id ?? null,
            clients: clientCountForTarget(target.key),
            playwright_cdp_endpoint: target.runtime.getCurrentEndpoint(),
            playwright_effective_cdp_endpoint: target.runtime.getEffectiveEndpoint(),
          })),
        }),
      );
      return;
    }

    if (url.pathname === "/control-session" && req.method === "GET") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          locked: managedSessionLocked,
          session: managedSession,
          cdp_endpoint: describeManagedCdpEndpoint(
            managedSession,
            automationAccessManager.getCached(managedSession),
          ),
          playwright_cdp_endpoint: playwrightRuntime.getCurrentEndpoint(),
          playwright_effective_cdp_endpoint: playwrightRuntime.getEffectiveEndpoint(),
        }),
      );
      return;
    }

    if (url.pathname === "/control-session" && req.method === "PUT") {
      if (managedSessionLocked) {
        res.writeHead(409, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "control session is locked by BPANE_SESSION_ID" }));
        return;
      }
      if (servers.size > 0) {
        res.writeHead(409, { "Content-Type": "application/json" });
        res.end(
          JSON.stringify({ error: "cannot switch control session while MCP clients are connected" }),
        );
        return;
      }
      try {
        const body = await readJsonBody<{ session_id?: string }>(req);
        const sessionId = (body.session_id ?? "").trim();
        if (!sessionId) {
          res.writeHead(400, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ error: "session_id is required" }));
          return;
        }
        const resolved = await sessionControlClient.getSession(sessionId);
        if (!resolved) {
          res.writeHead(404, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ error: `session ${sessionId} not found or not delegated` }));
          return;
        }
        const previousSession = managedSession;
        if (previousSession?.id && previousSession.id !== resolved.id) {
          await unregisterMcpOwner(previousSession, automationAccessManager);
        }
        await playwrightRuntime.close();
        setManagedSession(resolved);
        const nextAccess = await automationAccessManager.get(resolved);
        const nextCdpEndpoint = resolveManagedCdpEndpoint(resolved, nextAccess);
        console.log(
          `[mcp-bridge] control session set to ${resolved.id} (${resolved.state}) via ${nextCdpEndpoint}`,
        );
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ session: managedSession, cdp_endpoint: nextCdpEndpoint }));
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        res.writeHead(400, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: message }));
      }
      return;
    }

    if (url.pathname === "/control-session" && req.method === "DELETE") {
      if (managedSessionLocked) {
        res.writeHead(409, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "control session is locked by BPANE_SESSION_ID" }));
        return;
      }
      if (servers.size > 0) {
        res.writeHead(409, { "Content-Type": "application/json" });
        res.end(
          JSON.stringify({ error: "cannot clear control session while MCP clients are connected" }),
        );
        return;
      }
      await unregisterMcpOwner(managedSession, automationAccessManager);
      await playwrightRuntime.close();
      setManagedSession(null);
      console.log("[mcp-bridge] cleared control session");
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: true }));
      return;
    }

    if (isStreamableMcpPath(url.pathname)) {
      await handleStreamableHttpRequest(req, res, url);
      return;
    }

    if (isSsePath(url.pathname)) {
      let target: RuntimeTarget;
      try {
        target = await runtimeTargetForRequest(req, url);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        res.writeHead(404, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: message }));
        return;
      }
      const needsBridgeBootstrap = clientCountForTarget(target.key) === 0;
      try {
        if (needsBridgeBootstrap) {
          await registerMcpOwner(target.session, automationAccessManager, width, height);
        }
        const playwrightClient = await target.runtime.ensureConnected(
          target.session,
          automationAccessManager,
        );
        const transport = new SSEServerTransport("/messages", res);
        const sessionId = transport.sessionId;
        const server = createServerForConnection(playwrightClient, target.monitor);

        transports.set(sessionId, transport);
        servers.set(sessionId, server);
        mcpSessionTargetKeys.set(sessionId, target.key);

        console.log(
          `[mcp-bridge] MCP client connected (session=${sessionId}, bpane_target=${target.key}, total=${servers.size}, cdp=${target.runtime.getCurrentEndpoint() ?? "unknown"})`,
        );

        res.on("close", async () => {
          transports.delete(sessionId);
          servers.delete(sessionId);
          mcpSessionTargetKeys.delete(sessionId);
          console.log(`[mcp-bridge] MCP client disconnected (session=${sessionId}, remaining=${servers.size})`);
          await releaseOwnershipIfIdle(target);
        });

        await server.connect(transport);
      } catch (error) {
        if (needsBridgeBootstrap) {
          await unregisterMcpOwner(target.session, automationAccessManager);
        }
        const message = error instanceof Error ? error.message : String(error);
        res.writeHead(503, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: message }));
      }
      return;
    }

    if (url.pathname === "/messages") {
      const sessionId = url.searchParams.get("sessionId");
      if (!sessionId || !transports.has(sessionId)) {
        res.writeHead(400, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "invalid or missing sessionId" }));
        return;
      }
      const transport = transports.get(sessionId)!;
      await transport.handlePostMessage(req, res);
      return;
    }

    // POST /register — re-register MCP owner (e.g. after reconnect)
    if (url.pathname === "/register" && req.method === "POST") {
      try {
        await registerMcpOwner(managedSession, automationAccessManager, width, height);
        await playwrightRuntime.ensureConnected(managedSession, automationAccessManager);
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(
          JSON.stringify({
            ok: true,
            cdp_endpoint: playwrightRuntime.getCurrentEndpoint(),
            effective_cdp_endpoint: playwrightRuntime.getEffectiveEndpoint(),
          }),
        );
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        res.writeHead(503, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: message }));
      }
      return;
    }

    res.writeHead(404);
    res.end("not found");
  });

  httpServer.listen(MCP_PORT, "0.0.0.0", () => {
    console.log(`[mcp-bridge] MCP server listening on port ${MCP_PORT}`);
    console.log(`[mcp-bridge] Streamable HTTP endpoint: http://0.0.0.0:${MCP_PORT}/mcp`);
    console.log(`[mcp-bridge] SSE endpoint: http://0.0.0.0:${MCP_PORT}/sse`);
    console.log(`[mcp-bridge] Health: http://0.0.0.0:${MCP_PORT}/health`);
    console.log(`[mcp-bridge] Supervised delay: ${SUPERVISED_DELAY_MS}ms when viewers present`);
  });

  // Graceful shutdown
  const shutdown = async () => {
    console.log("\n[mcp-bridge] shutting down...");
    monitor.stop();
    await closeAllRuntimeTargets();
    httpServer.close();
    process.exit(0);
  };

  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);

  // Safety net: clear MCP ownership on unexpected crash
  process.on("uncaughtException", async (err) => {
    console.error("[mcp-bridge] uncaught exception:", err);
    await closeAllRuntimeTargets();
    process.exit(1);
  });

  process.on("unhandledRejection", async (reason) => {
    console.error("[mcp-bridge] unhandled rejection:", reason);
    await closeAllRuntimeTargets();
    process.exit(1);
  });
}

main().catch(async (err) => {
  console.error("[mcp-bridge] fatal:", err);
  try {
    const headers = await gatewayTokenManager.getAuthHeaders();
    await fetch(`${GATEWAY_API_URL}/api/session/mcp-owner`, {
      method: "DELETE",
      headers,
    });
  } catch {
    // ignore cleanup failures during fatal startup
  }
  process.exit(1);
});
