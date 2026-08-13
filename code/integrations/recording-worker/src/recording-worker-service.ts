import { promises as fs } from "node:fs";
import path from "node:path";
import { RecordingControlClient } from "./recording-control-client.js";
import { RecorderPageRuntime } from "./recorder-page-runtime.js";
import type { GatewayRecordingResource } from "./types.js";

const DEFAULT_MIN_CAPTURE_MS = 3000;

type RecorderConnectOptions = {
  gatewayUrl: string;
  transportPath: string;
  connectTicket: string;
};

type RecordingWorkerServiceOptions = {
  sessionId: string;
  recordingId: string;
  outputRoot: string;
  pollIntervalMs: number;
  minCaptureMs?: number;
  connect: RecorderConnectOptions | null;
  controlClient: RecordingControlPort;
  pageRuntime: RecorderPagePort;
};

type RecordingControlPort = Pick<
  RecordingControlClient,
  | "getRecording"
  | "createRecording"
  | "issueSessionAccessToken"
  | "completeRecording"
  | "failRecording"
>;

type RecorderPagePort = Pick<
  RecorderPageRuntime,
  "start" | "waitForMinimumCapture" | "stopAndDownload" | "close"
>;

export class RecordingWorkerService {
  private readonly sessionId: string;
  private readonly recordingId: string;
  private readonly outputRoot: string;
  private readonly pollIntervalMs: number;
  private readonly minCaptureMs: number;
  private readonly connect: RecorderConnectOptions | null;
  private readonly controlClient: RecordingControlPort;
  private readonly pageRuntime: RecorderPagePort;

  constructor(options: RecordingWorkerServiceOptions) {
    this.sessionId = options.sessionId;
    this.recordingId = options.recordingId.trim();
    this.outputRoot = options.outputRoot;
    this.pollIntervalMs = options.pollIntervalMs;
    const requestedMinCaptureMs = options.minCaptureMs ?? DEFAULT_MIN_CAPTURE_MS;
    this.minCaptureMs = Number.isFinite(requestedMinCaptureMs)
      ? Math.max(0, requestedMinCaptureMs)
      : DEFAULT_MIN_CAPTURE_MS;
    this.connect = options.connect;
    this.controlClient = options.controlClient;
    this.pageRuntime = options.pageRuntime;
  }

  async run(): Promise<void> {
    const recording = this.recordingId
      ? await this.controlClient.getRecording(this.sessionId, this.recordingId)
      : await this.controlClient.createRecording(this.sessionId);
    let artifactDownloaded = false;
    console.log(
      `[recording-worker] using recording ${recording.id} for session ${this.sessionId}`,
    );

    try {
      const connect = this.connect ?? await this.issueConnectTicket();
      await this.pageRuntime.start(connect);
      console.log(
        `[recording-worker] recorder client connected for session ${this.sessionId}`,
      );
      await this.pageRuntime.waitForMinimumCapture(this.minCaptureMs);
      const finalizationTarget = await this.waitForFinalize(recording.id);
      if (finalizationTarget.state === "ready") {
        console.log(
          `[recording-worker] recording ${recording.id} was already finalized elsewhere`,
        );
        return;
      }
      const outputPath = this.resolveArtifactPath(recording.id);
      const artifact = await this.pageRuntime.stopAndDownload(outputPath);
      artifactDownloaded = true;
      await this.controlClient.completeRecording(
        this.sessionId,
        recording.id,
        artifact.outputPath,
        artifact.mimeType,
        artifact.bytes,
        artifact.durationMs,
      );
      console.log(
        `[recording-worker] finalized recording ${recording.id} to ${artifact.outputPath}`,
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (!artifactDownloaded) {
        await fs.rm(this.resolveArtifactPath(recording.id), { force: true }).catch(() => {});
      }
      await this.controlClient
        .failRecording(this.sessionId, recording.id, message, "worker_exit")
        .catch(() => {});
      throw error;
    } finally {
      await this.pageRuntime.close();
    }
  }

  private async waitForFinalize(recordingId: string): Promise<GatewayRecordingResource> {
    for (;;) {
      const recording = await this.controlClient.getRecording(this.sessionId, recordingId);
      if (recording.state === "finalizing") {
        return recording;
      }
      if (recording.state === "ready") {
        return recording;
      }
      if (recording.state === "failed") {
        throw new Error(`recording ${recordingId} entered failed state`);
      }
      await this.sleep(this.pollIntervalMs);
    }
  }

  private resolveArtifactPath(recordingId: string): string {
    return path.join(this.outputRoot, this.sessionId, `${recordingId}.webm`);
  }

  private async issueConnectTicket(): Promise<RecorderConnectOptions> {
    const access = await this.controlClient.issueSessionAccessToken(this.sessionId);
    return {
      gatewayUrl: access.connect.gateway_url,
      transportPath: access.connect.transport_path,
      connectTicket: access.token,
    };
  }

  private async sleep(ms: number): Promise<void> {
    await new Promise((resolve) => setTimeout(resolve, ms));
  }
}
