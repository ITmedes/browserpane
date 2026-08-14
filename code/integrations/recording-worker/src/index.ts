import { GatewayTokenManager } from "./gateway-token-manager.js";
import { RecorderPageRuntime } from "./recorder-page-runtime.js";
import { RecordingControlClient } from "./recording-control-client.js";
import { RecordingWorkerService } from "./recording-worker-service.js";
import { WorkerSecretStore } from "./worker-secret-store.js";

const RECORDING_SECRET_KEYS = [
  "BPANE_RECORDING_BEARER_TOKEN",
  "BPANE_SESSION_AUTOMATION_ACCESS_TOKEN",
  "BPANE_RECORDING_WORKER_ACCESS_TOKEN",
  "BPANE_RECORDING_CONNECT_TICKET",
  "BPANE_GATEWAY_OIDC_CLIENT_SECRET",
] as const;

function requiredEnv(name: string): string {
  const value = (process.env[name] ?? "").trim();
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}

async function main(): Promise<void> {
  const secrets = await new WorkerSecretStore(RECORDING_SECRET_KEYS).load();
  const requestTimeoutMs = positiveIntegerEnv("BPANE_WORKER_REQUEST_TIMEOUT_MS", 30_000);
  const tokenManager = new GatewayTokenManager({
    staticBearerToken: secrets.BPANE_RECORDING_BEARER_TOKEN ?? "",
    tokenUrl: process.env.BPANE_GATEWAY_OIDC_TOKEN_URL ?? "",
    clientId: process.env.BPANE_GATEWAY_OIDC_CLIENT_ID ?? "",
    clientSecret: secrets.BPANE_GATEWAY_OIDC_CLIENT_SECRET ?? "",
    scopes: process.env.BPANE_GATEWAY_OIDC_SCOPES ?? "",
    requestTimeoutMs,
  });
  const controlClient = new RecordingControlClient({
    gatewayApiUrl: process.env.BPANE_GATEWAY_API_URL ?? "http://localhost:8932",
    sessionAutomationAccessToken: secrets.BPANE_SESSION_AUTOMATION_ACCESS_TOKEN ?? "",
    recordingWorkerAccessToken: requiredValue(
      secrets.BPANE_RECORDING_WORKER_ACCESS_TOKEN,
      "BPANE_RECORDING_WORKER_ACCESS_TOKEN",
    ),
    requestTimeoutMs,
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
    connect: resolveProvidedConnect(connectGatewayUrl, secrets),
    controlClient,
    pageRuntime,
  });

  await service.run();
}

function requiredValue(value: string | undefined, name: string): string {
  const resolved = (value ?? "").trim();
  if (!resolved) {
    throw new Error(`${name} is required`);
  }
  return resolved;
}

function positiveIntegerEnv(name: string, fallback: number): number {
  const raw = (process.env[name] ?? "").trim();
  if (!raw) {
    return fallback;
  }
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }
  return value;
}

main().catch((error) => {
  const message = error instanceof Error ? error.stack ?? error.message : String(error);
  console.error(`[recording-worker] ${message}`);
  process.exitCode = 1;
});

function resolveProvidedConnect(
  gatewayUrl: string,
  secrets: Readonly<Record<string, string>>,
): {
  gatewayUrl: string;
  transportPath: string;
  connectTicket: string;
} | null {
  const connectTicket = (secrets.BPANE_RECORDING_CONNECT_TICKET ?? "").trim();
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
