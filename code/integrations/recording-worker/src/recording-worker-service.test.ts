import assert from "node:assert/strict";
import test from "node:test";

import { RecordingWorkerService } from "./recording-worker-service.js";
import type { GatewayRecordingResource } from "./types.js";

test("polls recording state sequentially until finalization", async () => {
  let activePolls = 0;
  let maxActivePolls = 0;
  let getCalls = 0;
  let completed = false;
  const states: GatewayRecordingResource["state"][] = [
    "recording",
    "recording",
    "finalizing",
  ];
  const controlClient = {
    getRecording: async () => {
      activePolls += 1;
      maxActivePolls = Math.max(maxActivePolls, activePolls);
      const state = states[Math.min(getCalls, states.length - 1)]!;
      getCalls += 1;
      await new Promise((resolve) => setTimeout(resolve, 5));
      activePolls -= 1;
      return recording(state);
    },
    createRecording: async () => recording("recording"),
    issueSessionAccessToken: async () => {
      throw new Error("unexpected access-token request");
    },
    completeRecording: async () => {
      completed = true;
      return recording("ready");
    },
    failRecording: async () => recording("failed"),
  };
  const pageRuntime = {
    start: async () => {},
    waitForMinimumCapture: async () => {},
    stopAndDownload: async (outputPath: string) => ({
      outputPath,
      bytes: 100,
      mimeType: "video/webm",
      durationMs: 50,
    }),
    close: async () => {},
  };
  const service = new RecordingWorkerService({
    sessionId: "session-1",
    recordingId: "recording-1",
    outputRoot: "/tmp/recordings",
    pollIntervalMs: 0,
    minCaptureMs: 0,
    connect: {
      gatewayUrl: "https://gateway.test",
      transportPath: "/session",
      connectTicket: "ticket",
    },
    controlClient,
    pageRuntime,
  });

  await service.run();

  assert.equal(maxActivePolls, 1);
  assert.equal(getCalls, 3);
  assert.equal(completed, true);
});

function recording(state: GatewayRecordingResource["state"]): GatewayRecordingResource {
  return {
    id: "recording-1",
    session_id: "session-1",
    previous_recording_id: null,
    state,
    format: "webm",
    mime_type: null,
    bytes: null,
    duration_ms: null,
    error: null,
    termination_reason: null,
    artifact_available: false,
    content_path: "/recording",
    started_at: "2026-08-13T00:00:00Z",
    completed_at: null,
    created_at: "2026-08-13T00:00:00Z",
    updated_at: "2026-08-13T00:00:00Z",
  };
}
