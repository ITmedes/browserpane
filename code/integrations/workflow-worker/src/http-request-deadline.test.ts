import assert from "node:assert/strict";
import test from "node:test";

import { HttpRequestDeadline, WorkerRequestTimeoutError } from "./http-request-deadline.js";

test("rejects an operation at the configured deadline", async () => {
  const deadline = new HttpRequestDeadline(10);

  await assert.rejects(
    deadline.run("POST /safe-path", waitForAbort),
    (error: unknown) => {
      assert.ok(error instanceof WorkerRequestTimeoutError);
      assert.equal(error.code, "worker_request_timeout");
      assert.equal(error.operation, "POST /safe-path");
      assert.equal(error.timeoutMs, 10);
      return true;
    },
  );
});

test("preserves caller cancellation instead of translating it to a timeout", async () => {
  const deadline = new HttpRequestDeadline(1_000);
  const controller = new AbortController();
  const cancelled = new Error("cancelled by caller");
  const pending = deadline.run("POST /safe-path", waitForAbort, controller.signal);

  controller.abort(cancelled);

  await assert.rejects(pending, (error: unknown) => error === cancelled);
});

test("rejects invalid timeout configuration", () => {
  assert.throws(() => new HttpRequestDeadline(-1), /positive integer/u);
  assert.throws(() => new HttpRequestDeadline(1.5), /positive integer/u);
});

async function waitForAbort(signal: AbortSignal): Promise<never> {
  return new Promise((_resolve, reject) => {
    if (signal.aborted) {
      reject(signal.reason);
      return;
    }
    signal.addEventListener("abort", () => reject(signal.reason), { once: true });
  });
}
