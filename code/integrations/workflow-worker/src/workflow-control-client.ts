import type {
  GatewayAutomationTaskLogStream,
  GatewayResolvedWorkflowRunCredentialBinding,
  GatewaySessionAutomationAccessResponse,
  GatewayWorkflowDefinitionVersionResource,
  GatewayWorkflowRunProducedFileResource,
  GatewayWorkflowRunResource,
  WorkflowProducedFileUploadRequest,
} from "./types.js";
import { HttpRequestDeadline } from "./http-request-deadline.js";

type WorkflowControlClientOptions = {
  gatewayApiUrl: string;
  requestTimeoutMs: number;
  fetchImpl?: typeof fetch;
  getHeaders: (extraHeaders?: Record<string, string>) => Promise<Record<string, string>>;
};

type WorkflowRunStateUpdate = {
  state: string;
  output?: unknown;
  error?: string | null;
  artifact_refs?: string[];
  message?: string | null;
  data?: unknown;
};

export class WorkflowControlClient {
  private readonly gatewayApiUrl: string;
  private readonly deadline: HttpRequestDeadline;
  private readonly fetchImpl: typeof fetch;
  private readonly getHeaders: (
    extraHeaders?: Record<string, string>,
  ) => Promise<Record<string, string>>;

  constructor(options: WorkflowControlClientOptions) {
    this.gatewayApiUrl = options.gatewayApiUrl.replace(/\/$/, "");
    this.deadline = new HttpRequestDeadline(options.requestTimeoutMs);
    this.fetchImpl = options.fetchImpl ?? fetch;
    this.getHeaders = options.getHeaders;
  }

  getGatewayApiUrl(): string {
    return this.gatewayApiUrl;
  }

  async getWorkflowRun(runId: string): Promise<GatewayWorkflowRunResource> {
    return this.fetchJson<GatewayWorkflowRunResource>(
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}`,
    );
  }

  async getWorkflowDefinitionVersion(
    workflowId: string,
    version: string,
  ): Promise<GatewayWorkflowDefinitionVersionResource> {
    return this.fetchJson<GatewayWorkflowDefinitionVersionResource>(
      `/api/v1/workflows/${encodeURIComponent(workflowId)}/versions/${encodeURIComponent(version)}`,
    );
  }

  async issueAutomationAccess(
    sessionId: string,
  ): Promise<GatewaySessionAutomationAccessResponse> {
    return this.fetchJson<GatewaySessionAutomationAccessResponse>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-access`,
      { method: "POST" },
    );
  }

  async transitionWorkflowRun(
    runId: string,
    automationToken: string,
    request: WorkflowRunStateUpdate,
  ): Promise<GatewayWorkflowRunResource> {
    return this.fetchJsonWithAutomationAccess<GatewayWorkflowRunResource>(
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/state`,
      automationToken,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(request),
      },
    );
  }

  async appendWorkflowRunLog(
    runId: string,
    automationToken: string,
    stream: GatewayAutomationTaskLogStream,
    message: string,
  ): Promise<void> {
    await this.fetchJsonWithAutomationAccess(
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/logs`,
      automationToken,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ stream, message }),
      },
    );
  }

  async appendAutomationTaskLog(
    taskId: string,
    automationToken: string,
    stream: GatewayAutomationTaskLogStream,
    message: string,
  ): Promise<void> {
    await this.fetchJsonWithAutomationAccess(
      `/api/v1/automation-tasks/${encodeURIComponent(taskId)}/logs`,
      automationToken,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ stream, message }),
      },
    );
  }

  async downloadSourceSnapshot(runId: string, automationToken: string): Promise<Uint8Array> {
    return this.fetchBytesWithAutomationAccess(
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/source-snapshot/content`,
      automationToken,
    );
  }

  async resolveCredentialBinding(
    runId: string,
    bindingId: string,
    automationToken: string,
  ): Promise<GatewayResolvedWorkflowRunCredentialBinding> {
    return this.fetchJsonWithAutomationAccess<GatewayResolvedWorkflowRunCredentialBinding>(
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/credential-bindings/${encodeURIComponent(bindingId)}/resolved`,
      automationToken,
    );
  }

  async downloadWorkspaceInput(
    runId: string,
    inputId: string,
    automationToken: string,
  ): Promise<Uint8Array> {
    return this.fetchBytesWithAutomationAccess(
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/workspace-inputs/${encodeURIComponent(inputId)}/content`,
      automationToken,
    );
  }

  async uploadWorkflowRunProducedFile(
    runId: string,
    automationToken: string,
    request: WorkflowProducedFileUploadRequest,
  ): Promise<GatewayWorkflowRunProducedFileResource> {
    return this.fetchJsonWithAutomationAccess<GatewayWorkflowRunProducedFileResource>(
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/produced-files`,
      automationToken,
      {
        method: "POST",
        headers: {
          "Content-Type": request.mediaType ?? "application/octet-stream",
          "x-bpane-file-name": request.fileName,
          "x-bpane-workflow-workspace-id": request.workspaceId,
          ...(request.provenance
            ? {
                "x-bpane-file-provenance": JSON.stringify(request.provenance),
              }
            : {}),
        },
        body: Buffer.from(request.bytes),
      },
    );
  }

  private async fetchJson<T>(path: string, init: RequestInit = {}): Promise<T> {
    return this.fetchDecoded(path, init, async (response) => (await response.json()) as T);
  }

  private async fetchJsonWithAutomationAccess<T>(
    path: string,
    automationToken: string,
    init: RequestInit = {},
  ): Promise<T> {
    return this.fetchDecodedWithAutomationAccess(
      path,
      automationToken,
      init,
      async (response) => (await response.json()) as T,
    );
  }

  private async fetchBytesWithAutomationAccess(
    path: string,
    automationToken: string,
  ): Promise<Uint8Array> {
    return this.fetchDecodedWithAutomationAccess(
      path,
      automationToken,
      {},
      async (response) => new Uint8Array(await response.arrayBuffer()),
    );
  }

  private async fetchDecodedWithAutomationAccess<T>(
    path: string,
    automationToken: string,
    init: RequestInit,
    decode: (response: Response) => Promise<T>,
  ): Promise<T> {
    return this.fetchDecoded(path, {
      ...init,
      headers: {
        ...(init.headers as Record<string, string> | undefined),
        "x-bpane-automation-access-token": automationToken,
      },
    }, decode);
  }

  private async fetchDecoded<T>(
    path: string,
    init: RequestInit,
    decode: (response: Response) => Promise<T>,
  ): Promise<T> {
    const method = init.method?.toUpperCase() ?? "GET";
    return this.deadline.run(
      `${method} ${path}`,
      async (signal) => {
        const headers = await this.getHeaders({
          Accept: "application/json",
          ...(init.headers as Record<string, string> | undefined),
        });
        const response = await this.fetchImpl(`${this.gatewayApiUrl}${path}`, {
          ...init,
          headers,
          signal,
        });
        if (!response.ok) {
          let message = `${response.status} ${response.statusText}`.trim();
          try {
            const payload = (await response.json()) as { error?: string };
            if (payload?.error) {
              message = payload.error;
            }
          } catch {
            // Ignore malformed error bodies.
          }
          throw new WorkflowControlClientError(message, response.status);
        }
        return decode(response);
      },
      init.signal,
    );
  }
}

export class WorkflowControlClientError extends Error {
  readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "WorkflowControlClientError";
    this.status = status;
  }
}
