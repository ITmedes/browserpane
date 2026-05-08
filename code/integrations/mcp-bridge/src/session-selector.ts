import type { IncomingMessage } from "node:http";

export const BPANE_SESSION_ID_HEADER = "bpane-session-id";

const SESSION_SCOPED_PATH_RE = /^\/sessions\/([^/]+)\/(mcp|sse)$/;

export type SessionScopedPath = {
  sessionId: string;
  transport: "mcp" | "sse";
};

export function selectedBrowserPaneSessionId(
  req: IncomingMessage,
  url: URL,
): string | null {
  const pathValue = sessionScopedPath(url.pathname)?.sessionId;
  const headerValue = singleHeader(req.headers[BPANE_SESSION_ID_HEADER]);
  const queryValue = url.searchParams.get("bpaneSessionId");
  const candidates = [pathValue, headerValue, queryValue]
    .map((value) => value?.trim() ?? "")
    .filter((value) => value.length > 0);
  const selected = candidates[0] ?? "";
  if (candidates.some((value) => value !== selected)) {
    throw new Error("conflicting BrowserPane session selectors");
  }
  return selected.length > 0 ? selected : null;
}

export function isStreamableMcpPath(pathname: string): boolean {
  return pathname === "/mcp" || sessionScopedPath(pathname)?.transport === "mcp";
}

export function isSsePath(pathname: string): boolean {
  return pathname === "/sse" || sessionScopedPath(pathname)?.transport === "sse";
}

function sessionScopedPath(pathname: string): SessionScopedPath | null {
  const match = SESSION_SCOPED_PATH_RE.exec(pathname);
  if (!match) {
    return null;
  }
  try {
    return {
      sessionId: decodeURIComponent(match[1] ?? ""),
      transport: match[2] as "mcp" | "sse",
    };
  } catch {
    throw new Error("invalid BrowserPane session id in MCP path");
  }
}

function singleHeader(value: string | string[] | undefined): string | undefined {
  return Array.isArray(value) ? value[0] : value;
}
