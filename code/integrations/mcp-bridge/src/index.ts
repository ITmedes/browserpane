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
import { SupervisorMonitor } from "./supervisor-monitor.js";

// ── Configuration ────────────────────────────────────────────────────

const GATEWAY_API_URL = process.env.BPANE_GATEWAY_API_URL ?? "http://localhost:8932";
const MCP_PORT = parseInt(process.env.BPANE_MCP_PORT ?? "8931", 10);
const MCP_RESOLUTION = process.env.BPANE_MCP_RESOLUTION ?? "1600x900";
const CDP_ENDPOINT = process.env.BPANE_CDP_ENDPOINT ?? "http://127.0.0.1:9222";
const SUPERVISED_DELAY_MS = parseInt(process.env.BPANE_MCP_SUPERVISED_DELAY_MS ?? "1500", 10);
const POLL_INTERVAL_MS = parseInt(process.env.BPANE_MCP_POLL_INTERVAL_MS ?? "2000", 10);
const GATEWAY_OIDC_TOKEN_URL = process.env.BPANE_GATEWAY_OIDC_TOKEN_URL ?? "";
const GATEWAY_OIDC_CLIENT_ID = process.env.BPANE_GATEWAY_OIDC_CLIENT_ID ?? "";
const GATEWAY_OIDC_CLIENT_SECRET = process.env.BPANE_GATEWAY_OIDC_CLIENT_SECRET ?? "";
const GATEWAY_OIDC_SCOPES = process.env.BPANE_GATEWAY_OIDC_SCOPES ?? "";

// ── Helpers ──────────────────────────────────────────────────────────

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function parseResolution(s: string): { width: number; height: number } {
  const [w, h] = s.split("x").map(Number);
  if (!w || !h) throw new Error(`Invalid resolution: ${s}`);
  return { width: w, height: h };
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

// ── Register MCP owner with gateway ──────────────────────────────────

async function registerMcpOwner(width: number, height: number): Promise<void> {
  const maxRetries = 30;
  for (let i = 0; i < maxRetries; i++) {
    try {
      const headers = await gatewayTokenManager.getAuthHeaders({
        "Content-Type": "application/json",
      });
      const resp = await fetch(`${GATEWAY_API_URL}/api/session/mcp-owner`, {
        method: "POST",
        headers,
        body: JSON.stringify({ width, height }),
      });
      if (resp.ok) {
        console.log(`[mcp-bridge] registered as MCP owner at ${width}x${height}`);
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

async function unregisterMcpOwner(): Promise<void> {
  try {
    const headers = await gatewayTokenManager.getAuthHeaders();
    await fetch(`${GATEWAY_API_URL}/api/session/mcp-owner`, {
      method: "DELETE",
      headers,
    });
    console.log("[mcp-bridge] unregistered MCP owner");
  } catch {
    console.warn("[mcp-bridge] failed to unregister MCP owner (gateway unavailable)");
  }
}

// ── Spawn @playwright/mcp subprocess (STDIO mode) ───────────────────

function spawnPlaywrightMcp(): StdioClientTransport {
  return new StdioClientTransport({
    command: "npx",
    args: ["@playwright/mcp@latest", "--cdp-endpoint", CDP_ENDPOINT],
  });
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

  // 1. Do NOT register as MCP owner at startup — wait for first SSE client.
  //    This avoids locking the resolution when no MCP client is connected.

  // 2. Start supervisor monitor
  const monitor = new SupervisorMonitor(
    GATEWAY_API_URL,
    POLL_INTERVAL_MS,
    () => gatewayTokenManager.getAuthHeaders(),
  );
  monitor.start();

  // 3. Connect to @playwright/mcp subprocess
  const playwrightTransport = spawnPlaywrightMcp();
  const playwrightClient = new Client(
    { name: "bpane-mcp-bridge", version: "0.1.0" },
    { capabilities: {} },
  );
  await playwrightClient.connect(playwrightTransport);
  console.log("[mcp-bridge] connected to @playwright/mcp subprocess");

  // 4. Start HTTP/SSE server for external MCP clients.
  //    Each SSE connection gets its own Server instance to avoid the
  //    "Already connected to a transport" error.
  const transports = new Map<string, SSEServerTransport>();
  const servers = new Map<string, Server>();

  const httpServer = http.createServer(async (req, res) => {
    // CORS headers
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type");

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
        }),
      );
      return;
    }

    if (url.pathname === "/sse") {
      const transport = new SSEServerTransport("/messages", res);
      const sessionId = transport.sessionId;
      const server = createServerForConnection(playwrightClient, monitor);

      // Re-register MCP ownership when the first client connects after all disconnected
      if (servers.size === 0) {
        await registerMcpOwner(width, height);
      }

      transports.set(sessionId, transport);
      servers.set(sessionId, server);

      console.log(`[mcp-bridge] MCP client connected (session=${sessionId}, total=${servers.size})`);

      res.on("close", async () => {
        transports.delete(sessionId);
        servers.delete(sessionId);
        console.log(`[mcp-bridge] MCP client disconnected (session=${sessionId}, remaining=${servers.size})`);

        // When all MCP clients disconnect, clear ownership so resolution unlocks
        if (servers.size === 0) {
          console.log("[mcp-bridge] no MCP clients remaining — clearing ownership");
          await unregisterMcpOwner();
        }
      });

      await server.connect(transport);
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
      await registerMcpOwner(width, height);
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: true }));
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
    await unregisterMcpOwner();
    await playwrightClient.close();
    httpServer.close();
    process.exit(0);
  };

  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);

  // Safety net: clear MCP ownership on unexpected crash
  process.on("uncaughtException", async (err) => {
    console.error("[mcp-bridge] uncaught exception:", err);
    await unregisterMcpOwner();
    process.exit(1);
  });

  process.on("unhandledRejection", async (reason) => {
    console.error("[mcp-bridge] unhandled rejection:", reason);
    await unregisterMcpOwner();
    process.exit(1);
  });
}

main().catch(async (err) => {
  console.error("[mcp-bridge] fatal:", err);
  await unregisterMcpOwner();
  process.exit(1);
});
