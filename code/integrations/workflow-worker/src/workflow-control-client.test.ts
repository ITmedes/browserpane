import assert from "node:assert/strict";
import test from "node:test";

import { WorkerRequestTimeoutError } from "./http-request-deadline.js";
import { WorkflowControlClient } from "./workflow-control-client.js";

test("decodes binary workflow content within the request deadline", async () => {
  let capturedHeaders: Headers | undefined;
  const client = new WorkflowControlClient({
    gatewayApiUrl: "http://gateway.test/",
    requestTimeoutMs: 100,
    getHeaders: async (headers = {}) => ({ ...headers, Authorization: "Bearer owner" }),
    fetchImpl: async (_input, init) => {
      capturedHeaders = new Headers(init?.headers);
      return new Response(new Uint8Array([1, 2, 3]));
    },
  });

  const bytes = await client.downloadSourceSnapshot("run-1", "automation-secret");

  assert.deepEqual([...bytes], [1, 2, 3]);
  assert.equal(capturedHeaders?.get("authorization"), "Bearer owner");
  assert.equal(
    capturedHeaders?.get("x-bpane-automation-access-token"),
    "automation-secret",
  );
});

test("times out a stalled workflow control request", async () => {
  const client = new WorkflowControlClient({
    gatewayApiUrl: "http://gateway.test",
    requestTimeoutMs: 10,
    getHeaders: async (headers = {}) => headers,
    fetchImpl: async (_input, init) => waitForAbort(init?.signal),
  });

  await assert.rejects(
    client.getWorkflowRun("run-1"),
    (error: unknown) => {
      assert.ok(error instanceof WorkerRequestTimeoutError);
      assert.equal(error.operation, "GET /api/v1/workflow-runs/run-1");
      return true;
    },
  );
});

async function waitForAbort(signal?: AbortSignal | null): Promise<never> {
  return new Promise((_resolve, reject) => {
    if (signal?.aborted) {
      reject(signal.reason);
      return;
    }
    signal?.addEventListener("abort", () => reject(signal.reason), { once: true });
  });
}
