import { GatewayTokenManager } from "./gateway-token-manager.js";
import { WorkflowControlClient } from "./workflow-control-client.js";
import { WorkflowWorkerService } from "./workflow-worker-service.js";
import { WorkerSecretStore } from "./worker-secret-store.js";

const WORKFLOW_SECRET_KEYS = [
  "BPANE_SESSION_AUTOMATION_ACCESS_TOKEN",
  "BPANE_WORKFLOW_BEARER_TOKEN",
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
  const secrets = await new WorkerSecretStore(WORKFLOW_SECRET_KEYS).load();
  const requestTimeoutMs = positiveIntegerEnv("BPANE_WORKER_REQUEST_TIMEOUT_MS", 30_000);
  const tokenManager = new GatewayTokenManager({
    staticAutomationAccessToken: secrets.BPANE_SESSION_AUTOMATION_ACCESS_TOKEN ?? "",
    staticBearerToken: secrets.BPANE_WORKFLOW_BEARER_TOKEN ?? "",
    tokenUrl: process.env.BPANE_GATEWAY_OIDC_TOKEN_URL ?? "",
    clientId: process.env.BPANE_GATEWAY_OIDC_CLIENT_ID ?? "",
    clientSecret: secrets.BPANE_GATEWAY_OIDC_CLIENT_SECRET ?? "",
    scopes: process.env.BPANE_GATEWAY_OIDC_SCOPES ?? "",
    requestTimeoutMs,
  });
  const controlClient = new WorkflowControlClient({
    gatewayApiUrl: process.env.BPANE_GATEWAY_API_URL ?? "http://localhost:8932",
    requestTimeoutMs,
    getHeaders: (extraHeaders) => tokenManager.getAuthHeaders(extraHeaders),
  });
  const service = new WorkflowWorkerService({
    runId: requiredEnv("BPANE_WORKFLOW_RUN_ID"),
    workRoot: process.env.BPANE_WORKFLOW_WORK_ROOT ?? "/tmp/bpane-workflows",
    maxOutputBytes: positiveIntegerEnv("BPANE_WORKER_MAX_OUTPUT_BYTES", 262_144),
    controlClient,
  });

  await service.run();
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
  console.error(`[workflow-worker] ${message}`);
  process.exitCode = 1;
});
