import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { execFile as execFileCallback } from 'node:child_process';
import { promisify } from 'node:util';
import { fetchJson } from './workflow-smoke-lib.mjs';

const execFile = promisify(execFileCallback);

export async function createRecordingSession(accessToken, rootUrl) {
  return await fetchJson(`${rootUrl}/api/v1/sessions`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({
      owner_mode: 'collaborative',
      idle_timeout_sec: 300,
      recording: { mode: 'manual', format: 'webm' },
      labels: { suite: 'admin-recording-smoke' },
    }),
  });
}

export async function seedRetainedRecording(accessToken, rootUrl, sessionId, sourcePath, bytes) {
  const recording = await fetchJson(`${rootUrl}/api/v1/sessions/${sessionId}/recordings`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  await fetchJson(`${rootUrl}/api/v1/sessions/${sessionId}/recordings/${recording.id}/stop`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  const stageTarget = resolveGatewayVisiblePath(sessionId, recording.id);
  await stageFileForGateway(sourcePath, stageTarget);
  return await fetchJson(`${rootUrl}/api/v1/sessions/${sessionId}/recordings/${recording.id}/complete`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ source_path: stageTarget.gatewayPath, mime_type: 'video/webm', bytes, duration_ms: 1800 }),
  });
}

function resolveGatewayVisiblePath(sessionId, recordingId) {
  const hostRoot = process.env.BPANE_RECORDING_GATEWAY_STAGE_ROOT;
  const gatewayRoot = process.env.BPANE_RECORDING_GATEWAY_SOURCE_ROOT;
  const fileName = `browserpane-${sessionId}-${recordingId}-admin.webm`;
  if (hostRoot && gatewayRoot) {
    return {
      hostPath: path.join(hostRoot, 'admin-recording-smoke', fileName),
      gatewayPath: path.posix.join(gatewayRoot, 'admin-recording-smoke', fileName),
    };
  }
  return { gatewayPath: path.posix.join('/run/bpane', 'admin-recording-smoke', fileName) };
}

async function stageFileForGateway(sourcePath, target) {
  if (target.hostPath) {
    await fs.mkdir(path.dirname(target.hostPath), { recursive: true });
    await fs.copyFile(sourcePath, target.hostPath);
    return;
  }
  await execFile('docker', ['exec', 'deploy-gateway-1', 'mkdir', '-p', path.posix.dirname(target.gatewayPath)]);
  await execFile('docker', ['cp', sourcePath, `deploy-gateway-1:${target.gatewayPath}`]);
}
