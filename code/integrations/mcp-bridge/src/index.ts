import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { SSEServerTransport } from "@modelcontextprotocol/sdk/server/sse.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  ListResourcesRequestSchema,
  ListPromptsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import http from "node:http";
import {
  GatewaySessionAutomationAccessResponse,
  GatewaySessionResource,
  SessionControlClient,
} from "./session-control-client.js";
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
  private cachedAccess: GatewaySessionAutomationAccessResponse | null = null;

  constructor(private sessionControlClient: SessionControlClient) {}

  clear(sessionId?: string | null): void {
    if (!this.cachedAccess) {
      return;
    }
    if (sessionId && this.cachedAccess.session_id !== sessionId) {
      return;
    }
    this.cachedAccess = null;
  }

  getCached(session: GatewaySessionResource | null): GatewaySessionAutomationAccessResponse | null {
    if (!session || !this.cachedAccess) {
      return null;
    }
    if (this.cachedAccess.session_id !== session.id) {
      return null;
    }
    return this.cachedAccess;
  }

  async get(session: GatewaySessionResource | null): Promise<GatewaySessionAutomationAccessResponse | null> {
    if (!session) {
      this.cachedAccess = null;
      return null;
    }
    const now = Date.now();
    if (this.cachedAccess && this.cachedAccess.session_id === session.id) {
      const expiresAtMs = Date.parse(this.cachedAccess.expires_at);
      if (Number.isFinite(expiresAtMs) && now < expiresAtMs - 30_000) {
        return this.cachedAccess;
      }
    }
    const issued = await this.sessionControlClient.issueAutomationAccess(session.id);
    this.cachedAccess = issued;
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
  private cdpEndpoint: string | null = null;

  async ensureConnected(
    session: GatewaySessionResource | null,
    automationAccessManager: SessionAutomationAccessManager,
  ): Promise<Client> {
    const access = await automationAccessManager.get(session);
    const nextEndpoint = resolveManagedCdpEndpoint(session, access);
    if (this.client && this.cdpEndpoint === nextEndpoint) {
      return this.client;
    }

    await this.close();

    const transport = spawnPlaywrightMcp(nextEndpoint);
    const client = new Client(
      { name: "bpane-mcp-bridge", version: "0.1.0" },
      { capabilities: {} },
    );
    await client.connect(transport);
    this.client = client;
    this.cdpEndpoint = nextEndpoint;

    const sessionSuffix = session ? ` for session ${session.id}` : "";
    console.log(
      `[mcp-bridge] connected to @playwright/mcp subprocess${sessionSuffix} via ${nextEndpoint}`,
    );

    return client;
  }

  async close(): Promise<void> {
    if (!this.client) {
      this.cdpEndpoint = null;
      return;
    }

    const client = this.client;
    const previousEndpoint = this.cdpEndpoint;
    this.client = null;
    this.cdpEndpoint = null;

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
    return this.cdpEndpoint;
  }
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

  function setManagedSession(next: GatewaySessionResource | null): void {
    const previousSessionId = managedSession?.id ?? null;
    managedSession = next;
    automationAccessManager.clear(previousSessionId);
    monitor.setStatusPath(
      automationStatusPath(managedSession, automationAccessManager.getCached(managedSession)),
    );
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
  const servers = new Map<string, Server>();

  const httpServer = http.createServer(async (req, res) => {
    // CORS headers
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");

    if (req.method === "OPTIONS") {
      res.writeHead(204);
      res.end();
      return;
    }

    const url = new URL(req.url ?? "/", `http://localhost:${MCP_PORT}`);

    if (url.pathname === "/health") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          status: "ok",
          clients: servers.size,
          supervisors: monitor.getBrowserClientCount(),
          resolution: { width, height },
          supervised_delay_ms: SUPERVISED_DELAY_MS,
          control_session_id: managedSession?.id ?? null,
          control_session_state: managedSession?.state ?? null,
          control_session_mode: managedSession?.connect.compatibility_mode ?? null,
          control_session_cdp_endpoint: describeManagedCdpEndpoint(
            managedSession,
            automationAccessManager.getCached(managedSession),
          ),
          playwright_cdp_endpoint: playwrightRuntime.getCurrentEndpoint(),
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
        const nextAccess = await automationAccessManager.get(resolved);
        const nextCdpEndpoint = resolveManagedCdpEndpoint(resolved, nextAccess);
        const previousSession = managedSession;
        if (previousSession?.id && previousSession.id !== resolved.id) {
          await unregisterMcpOwner(previousSession, automationAccessManager);
        }
        await playwrightRuntime.close();
        setManagedSession(resolved);
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

    if (url.pathname === "/sse") {
      const needsBridgeBootstrap = servers.size === 0;
      try {
        if (needsBridgeBootstrap) {
          await registerMcpOwner(managedSession, automationAccessManager, width, height);
        }
        const playwrightClient = await playwrightRuntime.ensureConnected(
          managedSession,
          automationAccessManager,
        );
        const transport = new SSEServerTransport("/messages", res);
        const sessionId = transport.sessionId;
        const server = createServerForConnection(playwrightClient, monitor);

        transports.set(sessionId, transport);
        servers.set(sessionId, server);

        console.log(
          `[mcp-bridge] MCP client connected (session=${sessionId}, total=${servers.size}, cdp=${playwrightRuntime.getCurrentEndpoint() ?? "unknown"})`,
        );

        res.on("close", async () => {
          transports.delete(sessionId);
          servers.delete(sessionId);
          console.log(`[mcp-bridge] MCP client disconnected (session=${sessionId}, remaining=${servers.size})`);

          // When all MCP clients disconnect, clear ownership so resolution unlocks
          if (servers.size === 0) {
            console.log("[mcp-bridge] no MCP clients remaining — clearing ownership");
            await unregisterMcpOwner(managedSession, automationAccessManager);
            await playwrightRuntime.close();
          }
        });

        await server.connect(transport);
      } catch (error) {
        if (needsBridgeBootstrap) {
          await unregisterMcpOwner(managedSession, automationAccessManager);
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
    console.log(`[mcp-bridge] SSE endpoint: http://0.0.0.0:${MCP_PORT}/sse`);
    console.log(`[mcp-bridge] Health: http://0.0.0.0:${MCP_PORT}/health`);
    console.log(`[mcp-bridge] Supervised delay: ${SUPERVISED_DELAY_MS}ms when viewers present`);
  });

  // Graceful shutdown
  const shutdown = async () => {
    console.log("\n[mcp-bridge] shutting down...");
    monitor.stop();
    await unregisterMcpOwner(managedSession, automationAccessManager);
    await playwrightRuntime.close();
    httpServer.close();
    process.exit(0);
  };

  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);

  // Safety net: clear MCP ownership on unexpected crash
  process.on("uncaughtException", async (err) => {
    console.error("[mcp-bridge] uncaught exception:", err);
    await unregisterMcpOwner(managedSession, automationAccessManager);
    process.exit(1);
  });

  process.on("unhandledRejection", async (reason) => {
    console.error("[mcp-bridge] unhandled rejection:", reason);
    await unregisterMcpOwner(managedSession, automationAccessManager);
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
