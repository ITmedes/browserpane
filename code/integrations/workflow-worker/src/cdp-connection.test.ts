import assert from "node:assert/strict";
import test from "node:test";
import type { Browser } from "playwright-core";
import { connectWorkflowBrowser } from "./cdp-connection.js";

const browser = {} as Browser;

test("retries Docker DNS publication before connecting", async () => {
  let lookups = 0;
  const connected = await connectWorkflowBrowser({
    endpointUrl: "http://runtime-session:9223",
    authHeader: "x-bpane-token",
    authToken: "token",
    timeoutMs: 1_000,
    retryIntervalMs: 1,
    lookupImpl: async () => {
      lookups += 1;
      if (lookups < 3) throw Object.assign(new Error("not found"), { code: "ENOTFOUND" });
      return { address: "10.0.0.8" };
    },
    connectImpl: async (url, options) => {
      assert.equal(url, "http://10.0.0.8:9223/");
      assert.equal(options.headers["x-bpane-token"], "token");
      return browser;
    },
    sleepImpl: async () => {},
  });

  assert.equal(connected, browser);
  assert.equal(lookups, 3);
});

test("retries a refused CDP connection after DNS resolves", async () => {
  let connections = 0;
  const connected = await connectWorkflowBrowser({
    endpointUrl: "http://runtime-session:9223",
    authHeader: "x-bpane-token",
    authToken: "token",
    timeoutMs: 1_000,
    retryIntervalMs: 1,
    lookupImpl: async () => ({ address: "10.0.0.8" }),
    connectImpl: async () => {
      connections += 1;
      if (connections === 1) {
        throw Object.assign(new Error("connection refused"), { code: "ECONNREFUSED" });
      }
      return browser;
    },
    sleepImpl: async () => {},
  });

  assert.equal(connected, browser);
  assert.equal(connections, 2);
});

test("does not retry an authorization failure", async () => {
  let connections = 0;
  await assert.rejects(
    connectWorkflowBrowser({
      endpointUrl: "http://127.0.0.1:9223",
      authHeader: "x-bpane-token",
      authToken: "invalid",
      connectImpl: async () => {
        connections += 1;
        throw new Error("HTTP 401 Unauthorized");
      },
      sleepImpl: async () => {},
    }),
    /401 Unauthorized/u,
  );
  assert.equal(connections, 1);
});
