import assert from "node:assert/strict";
import test from "node:test";
import { buildManagedSessionHealth } from "../src/mcp-health.js";
import type { GatewaySessionResource } from "../src/session-control-client.js";

test("maps visible session-scoped MCP health", () => {
  const session = sessionResource("session-a", "active");
  const health = buildManagedSessionHealth({
    kind: "selected",
    session,
    visibleSession: session,
    clients: 2,
    backendDelegated: true,
    mcpOwner: true,
    cdpEndpoint: "ws://runtime-a:9222",
    playwrightCdpEndpoint: "ws://runtime-a:9222",
    playwrightEffectiveCdpEndpoint: "ws://runtime-a:9222",
    alignment: "aligned",
  });

  assert.deepEqual(health, {
    kind: "selected",
    session_id: "session-a",
    clients: 2,
    state: "active",
    mode: "session_runtime_pool",
    visible: true,
    backend_delegated: true,
    mcp_owner: true,
    cdp_endpoint: "ws://runtime-a:9222",
    playwright_cdp_endpoint: "ws://runtime-a:9222",
    playwright_effective_cdp_endpoint: "ws://runtime-a:9222",
    alignment: "aligned",
  });
});

test("keeps stale control-session health inspectable when hidden", () => {
  const health = buildManagedSessionHealth({
    kind: "control",
    session: sessionResource("session-b", "active"),
    visibleSession: null,
    clients: 0,
    backendDelegated: false,
    mcpOwner: null,
    cdpEndpoint: null,
    playwrightCdpEndpoint: null,
    playwrightEffectiveCdpEndpoint: null,
    alignment: "control_session_not_visible",
  });

  assert.equal(health.session_id, "session-b");
  assert.equal(health.state, "active");
  assert.equal(health.visible, false);
  assert.equal(health.backend_delegated, false);
  assert.equal(health.mcp_owner, null);
  assert.equal(health.alignment, "control_session_not_visible");
});

function sessionResource(id: string, state: GatewaySessionResource["state"]): GatewaySessionResource {
  return {
    id,
    state,
    owner_mode: "collaborative",
    automation_delegate: {
      client_id: "bpane-mcp-bridge",
      issuer: "http://localhost:8091/realms/bpane",
      display_name: "BrowserPane MCP bridge",
    },
    connect: {
      gateway_url: "https://localhost:4433",
      transport_path: "/session",
      auth_type: "session_connect_ticket",
      compatibility_mode: "session_runtime_pool",
    },
    runtime: {
      binding: "docker_runtime_pool",
      compatibility_mode: "session_runtime_pool",
      cdp_endpoint: "ws://runtime:9222",
    },
  };
}
