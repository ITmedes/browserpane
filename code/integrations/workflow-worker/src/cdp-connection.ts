import { lookup } from "node:dns/promises";
import net from "node:net";
import { chromium, type Browser } from "playwright-core";

type LookupResult = { address: string };

export type WorkflowBrowserConnectionOptions = {
  endpointUrl: string;
  authHeader: string;
  authToken: string;
  timeoutMs?: number;
  retryIntervalMs?: number;
  lookupImpl?: (hostname: string) => Promise<LookupResult>;
  connectImpl?: (
    endpointUrl: string,
    options: { headers: Record<string, string> },
  ) => Promise<Browser>;
  sleepImpl?: (milliseconds: number) => Promise<void>;
};

export async function connectWorkflowBrowser(
  options: WorkflowBrowserConnectionOptions,
): Promise<Browser> {
  const timeoutMs = options.timeoutMs ?? 60_000;
  const retryIntervalMs = options.retryIntervalMs ?? 500;
  const lookupImpl = options.lookupImpl ?? lookup;
  const connectImpl = options.connectImpl ?? ((endpointUrl, connectOptions) =>
    chromium.connectOverCDP(endpointUrl, connectOptions));
  const sleepImpl = options.sleepImpl ?? ((milliseconds) =>
    new Promise((resolve) => setTimeout(resolve, milliseconds)));
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown = null;

  do {
    try {
      const endpointUrl = await normalizeCdpEndpointUrl(options.endpointUrl, lookupImpl);
      return await connectImpl(endpointUrl, {
        headers: { [options.authHeader]: options.authToken },
      });
    } catch (error) {
      if (!isTransientConnectionError(error)) throw error;
      lastError = error;
      if (Date.now() + retryIntervalMs > deadline) break;
      await sleepImpl(retryIntervalMs);
    }
  } while (Date.now() < deadline);

  const detail = lastError instanceof Error ? lastError.message : String(lastError);
  throw new Error(`workflow browser did not become reachable within ${timeoutMs}ms: ${detail}`);
}

async function normalizeCdpEndpointUrl(
  endpointUrl: string,
  lookupImpl: (hostname: string) => Promise<LookupResult>,
): Promise<string> {
  const url = new URL(endpointUrl);
  if (url.hostname === "localhost" || net.isIP(url.hostname)) return url.toString();
  const resolved = await lookupImpl(url.hostname);
  url.hostname = resolved.address;
  return url.toString();
}

function isTransientConnectionError(error: unknown): boolean {
  const code = typeof error === "object" && error !== null && "code" in error
    ? String(error.code)
    : "";
  if (["EAI_AGAIN", "ECONNREFUSED", "ECONNRESET", "ENETUNREACH", "ENOTFOUND", "ETIMEDOUT"]
    .includes(code)) return true;
  const message = error instanceof Error ? error.message : String(error);
  return /EAI_AGAIN|ECONNREFUSED|ECONNRESET|ENETUNREACH|ENOTFOUND|ETIMEDOUT|socket hang up|HTTP (?:502|503|504)/iu
    .test(message);
}
