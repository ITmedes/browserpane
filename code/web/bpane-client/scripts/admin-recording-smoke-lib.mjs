import { fetchJson } from './workflow-smoke-lib.mjs';

export async function createRecordingSession(accessToken, rootUrl) {
  return await fetchJson(`${rootUrl}/api/v1/sessions`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({
      owner_mode: 'collaborative',
      idle_timeout_sec: 300,
      recording: { mode: 'always', format: 'webm' },
      labels: { suite: 'admin-recording-smoke' },
    }),
  });
}

export async function waitForActiveRecording(
  accessToken,
  rootUrl,
  sessionId,
  timeoutMs,
) {
  return await pollRecording(
    accessToken,
    rootUrl,
    sessionId,
    timeoutMs,
    (recording) => ['starting', 'recording'].includes(recording.state),
    'active recorder-worker segment',
  );
}

export async function disconnectAndWaitForRetainedRecording(
  accessToken,
  rootUrl,
  sessionId,
  timeoutMs,
) {
  await fetchJson(`${rootUrl}/api/v1/sessions/${sessionId}/connections/disconnect-all`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  return await pollRecording(
    accessToken,
    rootUrl,
    sessionId,
    timeoutMs,
    (recording) => recording.state === 'ready' && recording.artifact_available === true,
    'retained recorder-worker segment',
  );
}

async function pollRecording(
  accessToken,
  rootUrl,
  sessionId,
  timeoutMs,
  predicate,
  description,
) {
  const deadline = Date.now() + timeoutMs;
  let lastRecordings = [];
  while (Date.now() < deadline) {
    const catalog = await fetchJson(`${rootUrl}/api/v1/sessions/${sessionId}/recordings`, {
      headers: { Authorization: `Bearer ${accessToken}` },
    });
    lastRecordings = Array.isArray(catalog.recordings) ? catalog.recordings : [];
    const failed = lastRecordings.find((recording) => recording.state === 'failed');
    if (failed) {
      throw new Error(
        `Recorder worker failed for ${sessionId}: ${failed.error ?? 'unknown recording failure'}`,
      );
    }
    const match = lastRecordings.find(predicate);
    if (match) return match;
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(
    `Timed out waiting for ${description} for ${sessionId}: ${JSON.stringify(lastRecordings)}`,
  );
}
