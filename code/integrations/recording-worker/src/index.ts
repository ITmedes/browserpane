import { GatewayTokenManager } from "./gateway-token-manager.js";
import { RecorderPageRuntime } from "./recorder-page-runtime.js";
import { RecordingControlClient } from "./recording-control-client.js";
import { RecordingWorkerService } from "./recording-worker-service.js";

function requiredEnv(name: string): string {
  const value = (process.env[name] ?? "").trim();
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}

async function main(): Promise<void> {
  const tokenManager = new GatewayTokenManager({
    staticBearerToken: process.env.BPANE_RECORDING_BEARER_TOKEN ?? "",
    tokenUrl: process.env.BPANE_GATEWAY_OIDC_TOKEN_URL ?? "",
    clientId: process.env.BPANE_GATEWAY_OIDC_CLIENT_ID ?? "",
    clientSecret: process.env.BPANE_GATEWAY_OIDC_CLIENT_SECRET ?? "",
    scopes: process.env.BPANE_GATEWAY_OIDC_SCOPES ?? "",
  });
  const controlClient = new RecordingControlClient({
    gatewayApiUrl: process.env.BPANE_GATEWAY_API_URL ?? "http://localhost:8932",
    sessionAutomationAccessToken: process.env.BPANE_SESSION_AUTOMATION_ACCESS_TOKEN ?? "",
    getHeaders: (extraHeaders) => tokenManager.getAuthHeaders(extraHeaders),
  });
  const gatewayApiUrl = process.env.BPANE_GATEWAY_API_URL ?? "http://localhost:8932";
  const connectGatewayUrl = process.env.BPANE_RECORDING_CONNECT_GATEWAY_URL ?? deriveConnectGatewayUrl(gatewayApiUrl);
  const pageRuntime = new RecorderPageRuntime({
    pageUrl: process.env.BPANE_RECORDING_PAGE_URL ?? "http://localhost:8080",
    certSpki: process.env.BPANE_RECORDING_CERT_SPKI ?? process.env.BPANE_BENCHMARK_CERT_SPKI ?? "",
    chromeExecutablePath: requiredEnv("BPANE_RECORDING_CHROME"),
    connectGatewayUrl,
    connectTimeoutMs: Number.parseInt(process.env.BPANE_RECORDING_CONNECT_TIMEOUT_MS ?? "30000", 10),
    headless: (process.env.BPANE_RECORDING_HEADLESS ?? "true").trim().toLowerCase() !== "false",
  });
  const service = new RecordingWorkerService({
    sessionId: requiredEnv("BPANE_RECORDING_SESSION_ID"),
    recordingId: process.env.BPANE_RECORDING_ID ?? "",
    outputRoot: process.env.BPANE_RECORDING_OUTPUT_ROOT ?? "/tmp/bpane-recordings",
    pollIntervalMs: Number.parseInt(process.env.BPANE_RECORDING_POLL_INTERVAL_MS ?? "2000", 10),
    minCaptureMs: Number.parseInt(process.env.BPANE_RECORDING_MIN_CAPTURE_MS ?? "3000", 10),
    connect: resolveProvidedConnect(connectGatewayUrl),
    controlClient,
    pageRuntime,
  });

  await service.run();
}

main().catch((error) => {
  const message = error instanceof Error ? error.stack ?? error.message : String(error);
  console.error(`[recording-worker] ${message}`);
  process.exitCode = 1;
});

function resolveProvidedConnect(gatewayUrl: string): {
  gatewayUrl: string;
  transportPath: string;
  connectTicket: string;
} | null {
  const connectTicket = (process.env.BPANE_RECORDING_CONNECT_TICKET ?? "").trim();
  if (!connectTicket) {
    return null;
  }
  return {
    gatewayUrl,
    transportPath: (process.env.BPANE_RECORDING_CONNECT_TRANSPORT_PATH ?? "/session").trim() || "/session",
    connectTicket,
  };
}

function deriveConnectGatewayUrl(gatewayApiUrl: string): string {
  try {
    const url = new URL(gatewayApiUrl);
    return `https://${url.hostname}:4433`;
  } catch {
    return "https://localhost:4433";
  }
}
