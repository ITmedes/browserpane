import { timingSafeEqual } from "node:crypto";
import type http from "node:http";

export type ControlAuthDecision =
  | { readonly ok: true }
  | {
      readonly ok: false;
      readonly status: 401;
      readonly headers: Record<string, string>;
      readonly body: { readonly error: string };
    };

export function controlBearerTokenFromEnv(env: NodeJS.ProcessEnv): string | null {
  const value = (env.BPANE_MCP_BRIDGE_CONTROL_TOKEN ?? "").trim();
  return value.length > 0 ? value : null;
}

export function authorizeControlRequest(
  headers: http.IncomingHttpHeaders,
  expectedToken: string | null,
): ControlAuthDecision {
  if (!expectedToken) {
    return { ok: true };
  }

  const header = singleHeader(headers.authorization);
  const token = bearerToken(header);
  if (!token || !constantTimeEqual(token, expectedToken)) {
    return {
      ok: false,
      status: 401,
      headers: {
        "Content-Type": "application/json",
        "WWW-Authenticate": 'Bearer realm="bpane-mcp-bridge-control"',
      },
      body: { error: "MCP bridge control authorization is required" },
    };
  }

  return { ok: true };
}

function singleHeader(value: string | string[] | undefined): string | undefined {
  return Array.isArray(value) ? value[0] : value;
}

function bearerToken(header: string | undefined): string | null {
  if (!header) {
    return null;
  }
  const [scheme, token, extra] = header.trim().split(/\s+/);
  if (extra !== undefined || scheme?.toLowerCase() !== "bearer" || !token) {
    return null;
  }
  return token;
}

function constantTimeEqual(left: string, right: string): boolean {
  const leftBuffer = Buffer.from(left);
  const rightBuffer = Buffer.from(right);
  if (leftBuffer.length !== rightBuffer.length) {
    return false;
  }
  return timingSafeEqual(leftBuffer, rightBuffer);
}
