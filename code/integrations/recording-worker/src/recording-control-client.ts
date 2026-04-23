import type {
  GatewayRecordingResource,
  GatewayRecordingTerminationReason,
  GatewaySessionResource,
} from "./types.js";

type RecordingControlClientOptions = {
  gatewayApiUrl: string;
  getHeaders: (extraHeaders?: Record<string, string>) => Promise<Record<string, string>>;
};

export class RecordingControlClient {
  private readonly gatewayApiUrl: string;
  private readonly getHeaders: (
    extraHeaders?: Record<string, string>,
  ) => Promise<Record<string, string>>;

  constructor(options: RecordingControlClientOptions) {
    this.gatewayApiUrl = options.gatewayApiUrl.replace(/\/$/, "");
    this.getHeaders = options.getHeaders;
  }

  async getSession(sessionId: string): Promise<GatewaySessionResource> {
    return this.fetchJson<GatewaySessionResource>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}`,
    );
  }

  async createRecording(sessionId: string): Promise<GatewayRecordingResource> {
    return this.fetchJson<GatewayRecordingResource>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/recordings`,
      { method: "POST" },
    );
  }

  async getRecording(sessionId: string, recordingId: string): Promise<GatewayRecordingResource> {
    return this.fetchJson<GatewayRecordingResource>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/recordings/${encodeURIComponent(recordingId)}`,
    );
  }

  async completeRecording(
    sessionId: string,
    recordingId: string,
    sourcePath: string,
    mimeType: string,
    bytes: number,
    durationMs: number,
  ): Promise<GatewayRecordingResource> {
    return this.fetchJson<GatewayRecordingResource>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/recordings/${encodeURIComponent(recordingId)}/complete`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          source_path: sourcePath,
          mime_type: mimeType,
          bytes,
          duration_ms: durationMs,
        }),
      },
    );
  }

  async failRecording(
    sessionId: string,
    recordingId: string,
    error: string,
    terminationReason?: GatewayRecordingTerminationReason,
  ): Promise<GatewayRecordingResource> {
    const body: { error: string; termination_reason?: GatewayRecordingTerminationReason } = {
      error,
    };
    if (terminationReason) {
      body.termination_reason = terminationReason;
    }
    return this.fetchJson<GatewayRecordingResource>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/recordings/${encodeURIComponent(recordingId)}/fail`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      },
    );
  }

  private async fetchJson<T>(path: string, init: RequestInit = {}): Promise<T> {
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
      throw new RecordingControlClientError(message, response.status);
    }
    return (await response.json()) as T;
  }
}

export class RecordingControlClientError extends Error {
  readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "RecordingControlClientError";
    this.status = status;
  }
}
