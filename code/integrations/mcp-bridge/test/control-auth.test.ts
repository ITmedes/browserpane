import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  authorizeControlRequest,
  controlBearerTokenFromEnv,
} from "../src/control-auth.js";

describe("control auth", () => {
  it("allows local compatibility mode when no token is configured", () => {
    assert.deepEqual(authorizeControlRequest({}, null), { ok: true });
  });

  it("loads a trimmed bearer token from env", () => {
    assert.equal(
      controlBearerTokenFromEnv({ BPANE_MCP_BRIDGE_CONTROL_TOKEN: "  secret-value  " }),
      "secret-value",
    );
    assert.equal(controlBearerTokenFromEnv({ BPANE_MCP_BRIDGE_CONTROL_TOKEN: "   " }), null);
  });

  it("requires a matching bearer token when configured", () => {
    assert.equal(authorizeControlRequest({}, "secret-value").ok, false);
    assert.equal(
      authorizeControlRequest({ authorization: "Bearer wrong-value" }, "secret-value").ok,
      false,
    );
    assert.deepEqual(
      authorizeControlRequest({ authorization: "Bearer secret-value" }, "secret-value"),
      { ok: true },
    );
  });

  it("rejects malformed bearer headers", () => {
    assert.equal(
      authorizeControlRequest({ authorization: "Basic secret-value" }, "secret-value").ok,
      false,
    );
    assert.equal(
      authorizeControlRequest({ authorization: "Bearer secret-value extra" }, "secret-value").ok,
      false,
    );
  });
});
