import type { McpBridgeHealth } from './mcp-bridge-client';

export type McpBridgeHealthPollOptions = {
  readonly timeoutMs?: number;
  readonly intervalMs?: number;
};

export async function waitForMcpControlSession(
  loadHealth: () => Promise<McpBridgeHealth>,
  expectedSessionId: string | null,
  options: McpBridgeHealthPollOptions = {},
): Promise<McpBridgeHealth> {
  const timeoutMs = options.timeoutMs ?? 10_000;
  const intervalMs = options.intervalMs ?? 250;
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown = null;

  do {
    try {
      const health = await loadHealth();
      if (health.control_session_id === expectedSessionId) return health;
      lastError = new Error(
        `MCP bridge default is ${health.control_session_id ?? 'not set'}, expected ${expectedSessionId ?? 'not set'}.`,
      );
    } catch (error) {
      lastError = error;
    }
    if (Date.now() < deadline) await delay(intervalMs);
  } while (Date.now() < deadline);

  throw lastError instanceof Error
    ? lastError
    : new Error('MCP bridge default session did not converge before the timeout.');
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
