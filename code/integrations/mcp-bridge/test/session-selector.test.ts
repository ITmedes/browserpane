import assert from "node:assert/strict";
import type { IncomingHttpHeaders, IncomingMessage } from "node:http";
import test from "node:test";
import {
  BPANE_SESSION_ID_HEADER,
  isSsePath,
  isStreamableMcpPath,
  selectedBrowserPaneSessionId,
} from "../src/session-selector.js";

test("detects compatibility and session-scoped MCP paths", () => {
  assert.equal(isStreamableMcpPath("/mcp"), true);
  assert.equal(isStreamableMcpPath("/sessions/session-a/mcp"), true);
  assert.equal(isStreamableMcpPath("/sessions/session-a/sse"), false);
  assert.equal(isSsePath("/sse"), true);
  assert.equal(isSsePath("/sessions/session-a/sse"), true);
  assert.equal(isSsePath("/sessions/session-a/mcp"), false);
});

test("resolves a session selector from path, header, or query", () => {
  assert.equal(selectedBrowserPaneSessionId(request(), url("/sessions/session-a/mcp")), "session-a");
  assert.equal(
    selectedBrowserPaneSessionId(request({ [BPANE_SESSION_ID_HEADER]: "session-b" }), url("/mcp")),
    "session-b",
  );
  assert.equal(
    selectedBrowserPaneSessionId(request(), url("/sse?bpaneSessionId=session-c")),
    "session-c",
  );
});

test("accepts duplicate selectors when they agree", () => {
  const selected = selectedBrowserPaneSessionId(
    request({ [BPANE_SESSION_ID_HEADER]: "session-a" }),
    url("/sessions/session-a/mcp?bpaneSessionId=session-a"),
  );

  assert.equal(selected, "session-a");
});

test("rejects conflicting or malformed session selectors", () => {
  assert.throws(
    () => selectedBrowserPaneSessionId(
      request({ [BPANE_SESSION_ID_HEADER]: "session-b" }),
      url("/sessions/session-a/mcp"),
    ),
    /conflicting BrowserPane session selectors/,
  );
  assert.throws(
    () => selectedBrowserPaneSessionId(request(), url("/sessions/%E0%A4%A/mcp")),
    /invalid BrowserPane session id/,
  );
});

function request(headers: IncomingHttpHeaders = {}): IncomingMessage {
  return { headers } as IncomingMessage;
}

function url(path: string): URL {
  return new URL(path, "http://localhost:8931");
}
