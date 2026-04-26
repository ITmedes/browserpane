import { GatewayTokenManager } from "./gateway-token-manager.js";
import { WorkflowControlClient } from "./workflow-control-client.js";
import { WorkflowWorkerService } from "./workflow-worker-service.js";

function requiredEnv(name: string): string {
  const value = (process.env[name] ?? "").trim();
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}

async function main(): Promise<void> {
  const tokenManager = new GatewayTokenManager({
    staticAutomationAccessToken: process.env.BPANE_SESSION_AUTOMATION_ACCESS_TOKEN ?? "",
    staticBearerToken: process.env.BPANE_WORKFLOW_BEARER_TOKEN ?? "",
    tokenUrl: process.env.BPANE_GATEWAY_OIDC_TOKEN_URL ?? "",
    clientId: process.env.BPANE_GATEWAY_OIDC_CLIENT_ID ?? "",
    clientSecret: process.env.BPANE_GATEWAY_OIDC_CLIENT_SECRET ?? "",
    scopes: process.env.BPANE_GATEWAY_OIDC_SCOPES ?? "",
  });
  const controlClient = new WorkflowControlClient({
    gatewayApiUrl: process.env.BPANE_GATEWAY_API_URL ?? "http://localhost:8932",
    getHeaders: (extraHeaders) => tokenManager.getAuthHeaders(extraHeaders),
  });
  const service = new WorkflowWorkerService({
    runId: requiredEnv("BPANE_WORKFLOW_RUN_ID"),
    workRoot: process.env.BPANE_WORKFLOW_WORK_ROOT ?? "/tmp/bpane-workflows",
    controlClient,
  });

  await service.run();
}

main().catch((error) => {
  const message = error instanceof Error ? error.stack ?? error.message : String(error);
  console.error(`[workflow-worker] ${message}`);
  process.exitCode = 1;
});
