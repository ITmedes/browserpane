import type {
  GatewayAutomationTaskLogStream,
  GatewaySessionAutomationAccessResponse,
  GatewayWorkflowDefinitionVersionResource,
  GatewayWorkflowRunResource,
} from "./types.js";

type WorkflowControlClientOptions = {
  gatewayApiUrl: string;
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
  private readonly getHeaders: (
    extraHeaders?: Record<string, string>,
  ) => Promise<Record<string, string>>;

  constructor(options: WorkflowControlClientOptions) {
    this.gatewayApiUrl = options.gatewayApiUrl.replace(/\/$/, "");
    this.getHeaders = options.getHeaders;
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
    const response = await this.fetchResponseWithAutomationAccess(
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/source-snapshot/content`,
      automationToken,
    );
    return new Uint8Array(await response.arrayBuffer());
  }

  async downloadWorkspaceInput(
    runId: string,
    inputId: string,
    automationToken: string,
  ): Promise<Uint8Array> {
    const response = await this.fetchResponseWithAutomationAccess(
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/workspace-inputs/${encodeURIComponent(inputId)}/content`,
      automationToken,
    );
    return new Uint8Array(await response.arrayBuffer());
  }

  private async fetchJson<T>(path: string, init: RequestInit = {}): Promise<T> {
    const response = await this.fetchResponse(path, init);
    return (await response.json()) as T;
  }

  private async fetchJsonWithAutomationAccess<T>(
    path: string,
    automationToken: string,
    init: RequestInit = {},
  ): Promise<T> {
    const response = await this.fetchResponseWithAutomationAccess(path, automationToken, init);
    return (await response.json()) as T;
  }

  private async fetchResponseWithAutomationAccess(
    path: string,
    automationToken: string,
    init: RequestInit = {},
  ): Promise<Response> {
    return this.fetchResponse(path, {
      ...init,
      headers: {
        ...(init.headers as Record<string, string> | undefined),
        "x-bpane-automation-access-token": automationToken,
      },
    });
  }

  private async fetchResponse(path: string, init: RequestInit = {}): Promise<Response> {
    const headers = await this.getHeaders({
      Accept: "application/json",
      ...(init.headers as Record<string, string> | undefined),
    });
    const response = await fetch(`${this.gatewayApiUrl}${path}`, {
      ...init,
      headers,
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
    return response;
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
