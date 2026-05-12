import { fetchJson } from './workflow-smoke-lib.mjs';

export async function createWorkflow(accessToken, rootUrl) {
  return await fetchJson(`${rootUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: jsonAuthHeaders(accessToken),
    body: JSON.stringify({
      name: `admin-workflow-smoke-${Date.now()}`,
      description: 'Validate admin workflow operations controls',
      labels: {
        suite: 'admin-workflow-smoke',
        bpane_admin_hidden: 'true',
      },
    }),
  });
}

export async function createWorkflowVersion(accessToken, rootUrl, workflowId) {
  return await fetchJson(`${rootUrl}/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    headers: jsonAuthHeaders(accessToken),
    body: JSON.stringify({
      version: 'v1',
      executor: 'manual',
      entrypoint: 'workflows/operator/run.mjs',
    }),
  });
}

export async function issueAutomationAccess(accessToken, rootUrl, sessionId) {
  return await fetchJson(`${rootUrl}/api/v1/sessions/${sessionId}/automation-access`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

export async function transitionRun(token, rootUrl, runId, body) {
  return await fetchJson(`${rootUrl}/api/v1/workflow-runs/${runId}/state`, {
    method: 'POST',
    headers: automationHeaders(token),
    body: JSON.stringify(body),
  });
}

export async function appendRunLog(token, rootUrl, runId, message) {
  return await fetchJson(`${rootUrl}/api/v1/workflow-runs/${runId}/logs`, {
    method: 'POST',
    headers: automationHeaders(token),
    body: JSON.stringify({ stream: 'stdout', message }),
  });
}

function jsonAuthHeaders(accessToken) {
  return { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' };
}

function automationHeaders(token) {
  return { 'x-bpane-automation-access-token': token, 'Content-Type': 'application/json' };
}
