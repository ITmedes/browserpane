import assert from "node:assert/strict";
import test from "node:test";

import { GatewayTokenManager } from "./gateway-token-manager.js";
import { WorkerRequestTimeoutError } from "./http-request-deadline.js";

test("coalesces concurrent OIDC token requests and reuses the token", async () => {
  let requests = 0;
  const manager = new GatewayTokenManager({
    staticAutomationAccessToken: "",
    staticBearerToken: "",
    tokenUrl: "http://identity.test/token",
    clientId: "worker",
    clientSecret: "secret",
    scopes: "gateway",
    requestTimeoutMs: 100,
    fetchImpl: async () => {
      requests += 1;
      await new Promise((resolve) => setTimeout(resolve, 5));
      return Response.json({ access_token: "issued-token", expires_in: 60 });
    },
  });

  const [first, second] = await Promise.all([
    manager.getAuthHeaders(),
    manager.getAuthHeaders({ Accept: "application/json" }),
  ]);
  const third = await manager.getAuthHeaders();

  assert.equal(requests, 1);
  assert.equal(first.Authorization, "Bearer issued-token");
  assert.equal(second.Authorization, "Bearer issued-token");
  assert.equal(third.Authorization, "Bearer issued-token");
});

test("applies the request deadline to OIDC token acquisition", async () => {
  const manager = new GatewayTokenManager({
    staticAutomationAccessToken: "",
    staticBearerToken: "",
    tokenUrl: "http://identity.test/token",
    clientId: "worker",
    clientSecret: "secret",
    scopes: "",
    requestTimeoutMs: 10,
    fetchImpl: async (_input, init) => waitForAbort(init?.signal),
  });

  await assert.rejects(
    manager.getAuthHeaders(),
    (error: unknown) => error instanceof WorkerRequestTimeoutError,
  );
});

async function waitForAbort(signal?: AbortSignal | null): Promise<never> {
  return new Promise((_resolve, reject) => {
    signal?.addEventListener("abort", () => reject(signal.reason), { once: true });
  });
}
