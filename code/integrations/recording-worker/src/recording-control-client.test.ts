import assert from "node:assert/strict";
import test from "node:test";

import { WorkerRequestTimeoutError } from "./http-request-deadline.js";
import { RecordingControlClient } from "./recording-control-client.js";

test("sends scoped automation headers and decodes a recording", async () => {
  let capturedHeaders: Headers | undefined;
  const client = new RecordingControlClient({
    gatewayApiUrl: "http://gateway.test/",
    sessionAutomationAccessToken: "automation-secret",
    recordingWorkerAccessToken: "worker-secret",
    requestTimeoutMs: 100,
    getHeaders: async (headers = {}) => ({ ...headers, Authorization: "Bearer owner" }),
    fetchImpl: async (_input, init) => {
      capturedHeaders = new Headers(init?.headers);
      return Response.json({ id: "recording-1", state: "recording" });
    },
  });

  const recording = await client.getRecording("session-1", "recording-1");

  assert.equal(recording.id, "recording-1");
  assert.equal(capturedHeaders?.get("authorization"), "Bearer owner");
  assert.equal(
    capturedHeaders?.get("x-bpane-automation-access-token"),
    "automation-secret",
  );
});

test("times out a stalled gateway request without exposing credentials", async () => {
  const client = new RecordingControlClient({
    gatewayApiUrl: "http://gateway.test",
    sessionAutomationAccessToken: "automation-secret",
    recordingWorkerAccessToken: "worker-secret",
    requestTimeoutMs: 10,
    getHeaders: async (headers = {}) => headers,
    fetchImpl: async (_input, init) => waitForAbort(init?.signal),
  });

  await assert.rejects(
    client.getRecording("session-1", "recording-1"),
    (error: unknown) => {
      assert.ok(error instanceof WorkerRequestTimeoutError);
      assert.match(error.message, /GET \/api\/v1\/sessions\/session-1\/recordings\/recording-1/u);
      assert.doesNotMatch(error.message, /secret/u);
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
