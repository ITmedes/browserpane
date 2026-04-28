import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';

import { chromium } from 'playwright-core';

import {
  buildWorkflowWorkerImage,
  cleanupWorkflowSmokeSessions,
  configurePage,
  createLocalWorkflowRepo,
  createLogger,
  deleteSession,
  ensureLoggedIn,
  fetchJson,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

const log = createLogger('workflow-cli-smoke');

function workflowCliEntrypoint() {
  return `export default async function run({ page, artifacts, input, sessionId, sourceRoot }) {
  const targetUrl =
    input && typeof input.target_url === 'string' && input.target_url.trim()
      ? input.target_url.trim()
      : 'http://web:8080';
  const workspaceId =
    input && typeof input.workspace_id === 'string' && input.workspace_id.trim()
      ? input.workspace_id.trim()
      : '';
  if (!workspaceId) {
    throw new Error('workflow cli smoke requires workspace_id input');
  }
  await page.goto(targetUrl, { waitUntil: 'networkidle' });
  const artifactText = \`workflow cli artifact for \${sessionId}\`;
  const uploaded = await artifacts.uploadTextFile({
    workspaceId,
    fileName: 'cli-output.txt',
    text: artifactText,
    mediaType: 'text/plain',
    provenance: {
      suite: 'workflow-cli-smoke',
      source_root: sourceRoot,
    },
  });
  console.log(\`workflow cli uploaded \${uploaded.file_name}\`);
  return {
    title: await page.title(),
    final_url: page.url(),
    session_id: sessionId,
    produced_file_id: uploaded.file_id,
    artifact_text: artifactText,
  };
}
`;
}

async function createFileWorkspace(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/file-workspaces`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-cli-smoke-workspace',
      labels: {
        suite: 'workflow-cli-smoke',
      },
    }),
  });
}

async function issueAutomationAccess(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}/automation-access`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function transitionRun(automationToken, options, runId, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/state`, {
    method: 'POST',
    headers: {
      'x-bpane-automation-access-token': automationToken,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

function runWorkflowCli({ args, cwd, env }) {
  const cliPath = path.join(cwd, 'scripts', 'workflow-cli.mjs');
  const result = spawnSync(process.execPath, [cliPath, ...args], {
    cwd,
    env,
    encoding: 'utf8',
  });
  if (result.status !== 0) {
    const detail = result.error?.message ?? result.stderr ?? result.stdout;
    throw new Error(
      `workflow CLI failed with code ${result.status ?? 'unknown'}: ${detail}`,
    );
  }
  const stdout = result.stdout.trim();
  return stdout ? JSON.parse(stdout) : null;
}

async function main() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-cli-smoke.mjs');
  const browser = await launchChrome(chromium, options);

  let context = null;
  let page = null;
  let accessToken = '';
  let localWorkflowSource = null;

  try {
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
      deviceScaleFactor: 1,
    });
    page = await context.newPage();
    await configurePage(page, options);
    await ensureLoggedIn(page, options);
    accessToken = (await getAccessToken(page)) ?? '';
    if (!accessToken) {
      throw new Error('Failed to acquire an access token from the test page.');
    }

    await cleanupWorkflowSmokeSessions(accessToken, options, log);

    log('Preparing local git-backed workflow source');
    localWorkflowSource = await createLocalWorkflowRepo('.workflow-cli-smoke-repo-', {
      'workflows/cli/run.mjs': workflowCliEntrypoint(),
    });

    log('Building workflow-worker image');
    buildWorkflowWorkerImage();

    const workspace = await createFileWorkspace(accessToken, options);
    if (!workspace.id) {
      throw new Error('Workflow CLI smoke workspace creation did not return an id.');
    }

    const cliCwd = process.cwd();
    const cliEnv = {
      ...process.env,
      BPANE_API_URL: options.pageUrl,
      BPANE_ACCESS_TOKEN: accessToken,
    };

    const workflow = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'create',
        '--body-json',
        JSON.stringify({
          name: 'workflow-cli-smoke',
          description: 'Validate the workflow CLI against the owner-scoped API',
          labels: {
            suite: 'workflow-cli-smoke',
          },
        }),
      ],
    });

    const version = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'version',
        'create',
        workflow.id,
        '--body-json',
        JSON.stringify({
          version: 'v1',
          executor: 'playwright',
          entrypoint: 'workflows/cli/run.mjs',
          source: {
            kind: 'git',
            repository_url: localWorkflowSource.repositoryUrl,
            ref: 'refs/heads/main',
            root_path: 'workflows',
          },
          input_schema: {
            type: 'object',
            required: ['target_url', 'workspace_id'],
            properties: {
              target_url: { type: 'string' },
              workspace_id: { type: 'string' },
            },
          },
          default_session: {
            labels: {
              origin: 'workflow-cli-smoke',
            },
          },
          allowed_file_workspace_ids: [workspace.id],
        }),
      ],
    });
    if (version.source?.resolved_commit !== localWorkflowSource.commit) {
      throw new Error('Workflow CLI smoke did not pin the expected git commit.');
    }

    const workflowLookup = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: ['workflow', 'get', workflow.id],
    });
    if (workflowLookup.id !== workflow.id) {
      throw new Error('Workflow CLI smoke failed to load the created workflow.');
    }

    const clientRequestId = `workflow-cli-smoke-job-${crypto.randomUUID()}`;
    const runRequest = {
      workflow_id: workflow.id,
      version: 'v1',
      source_system: 'camunda-prod',
      source_reference: 'process-instance-123/task-7',
      client_request_id: clientRequestId,
      input: {
        target_url: 'http://web:8080',
        workspace_id: workspace.id,
      },
      labels: {
        suite: 'workflow-cli-smoke',
      },
    };
    const run = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'run',
        'create',
        '--body-json',
        JSON.stringify(runRequest),
      ],
    });
    if (!run.id) {
      throw new Error('Workflow CLI smoke run creation did not return an id.');
    }
    if (run.source_system !== 'camunda-prod') {
      throw new Error('Workflow CLI smoke did not persist source_system on the created run.');
    }
    if (run.source_reference !== 'process-instance-123/task-7') {
      throw new Error('Workflow CLI smoke did not persist source_reference on the created run.');
    }
    if (run.client_request_id !== clientRequestId) {
      throw new Error('Workflow CLI smoke did not persist client_request_id on the created run.');
    }

    const duplicateRun = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'run',
        'create',
        '--body-json',
        JSON.stringify(runRequest),
      ],
    });
    if (duplicateRun.id !== run.id) {
      throw new Error('Workflow CLI smoke idempotent retry returned a different workflow run id.');
    }
    if (duplicateRun.session_id !== run.session_id) {
      throw new Error('Workflow CLI smoke idempotent retry returned a different session id.');
    }

    const waitedRun = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'run',
        'wait',
        run.id,
        '--target-state',
        'succeeded',
        '--timeout-ms',
        String(options.connectTimeoutMs),
      ],
    });
    if (waitedRun.state !== 'succeeded') {
      throw new Error(`Workflow CLI smoke expected succeeded run, got ${waitedRun.state ?? 'unknown'}.`);
    }

    const logs = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: ['workflow', 'run', 'logs', run.id],
    });
    if (
      !logs.logs.some(
        (entry) =>
          entry.source === 'automation_task' &&
          entry.message.includes('workflow cli uploaded cli-output.txt'),
      )
    ) {
      throw new Error('Workflow CLI smoke is missing the produced-file log line.');
    }

    const producedFiles = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: ['workflow', 'run', 'produced-files', run.id],
    });
    const producedFile = producedFiles.files?.[0];
    if (!producedFile?.file_id) {
      throw new Error('Workflow CLI smoke did not expose a produced file.');
    }

    const downloadDir = await fs.mkdtemp(path.join(os.tmpdir(), '.workflow-cli-download-'));
    const downloadPath = path.join(downloadDir, 'cli-output.txt');
    const downloadSummary = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'run',
        'download-produced-file',
        run.id,
        producedFile.file_id,
        '--output',
        downloadPath,
      ],
    });
    const downloadedText = await fs.readFile(downloadPath, 'utf8');
    if (!downloadedText.includes(waitedRun.output?.artifact_text ?? '')) {
      throw new Error('Workflow CLI smoke downloaded content does not match the workflow output.');
    }

    if (waitedRun.session_id) {
      await deleteSession(accessToken, options, waitedRun.session_id);
    }

    log('Validating workflow CLI intervention commands');
    const manualWorkflow = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'create',
        '--body-json',
        JSON.stringify({
          name: 'workflow-cli-intervention-smoke',
          description: 'Validate workflow CLI operator actions',
          labels: {
            suite: 'workflow-cli-smoke',
            mode: 'intervention',
          },
        }),
      ],
    });

    const manualVersion = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'version',
        'create',
        manualWorkflow.id,
        '--body-json',
        JSON.stringify({
          version: 'v1',
          executor: 'manual',
          entrypoint: 'workflows/operator/run.mjs',
        }),
      ],
    });
    if (manualVersion.executor !== 'manual') {
      throw new Error('Workflow CLI smoke failed to create the manual workflow version.');
    }

    const manualRun = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'run',
        'create',
        '--body-json',
        JSON.stringify({
          workflow_id: manualWorkflow.id,
          version: 'v1',
          session: {
            create_session: {},
          },
          labels: {
            suite: 'workflow-cli-smoke',
            mode: 'intervention',
          },
        }),
      ],
    });
    if (!manualRun.id || !manualRun.session_id) {
      throw new Error('Workflow CLI smoke manual run creation did not return run and session ids.');
    }

    const automationAccess = await issueAutomationAccess(accessToken, options, manualRun.session_id);
    const automationToken = automationAccess.token ?? '';
    if (!automationToken) {
      throw new Error('Workflow CLI smoke failed to issue automation access for the manual run.');
    }

    await transitionRun(automationToken, options, manualRun.id, {
      state: 'running',
      message: 'executor attached',
    });

    const firstRequestId = crypto.randomUUID();
    await transitionRun(automationToken, options, manualRun.id, {
      state: 'awaiting_input',
      message: 'approval required',
      data: {
        intervention_request: {
          request_id: firstRequestId,
          kind: 'approval',
          prompt: 'Approve the CLI smoke',
        },
        runtime_hold: {
          mode: 'live',
          timeout_sec: 5,
        },
      },
    });

    const awaitedManualRun = await poll(
      'manual workflow run awaiting input',
      async () =>
        runWorkflowCli({
          cwd: cliCwd,
          env: cliEnv,
          args: ['workflow', 'run', 'get', manualRun.id],
        }),
      (value) => value?.state === 'awaiting_input' && value?.runtime?.resume_mode === 'live_runtime',
      options.connectTimeoutMs,
      250,
    );
    if (awaitedManualRun.intervention?.pending_request?.request_id !== firstRequestId) {
      throw new Error('Workflow CLI smoke did not expose the pending intervention request.');
    }

    const submittedRun = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'run',
        'submit-input',
        manualRun.id,
        '--body-json',
        JSON.stringify({
          input: {
            approved: true,
          },
          comment: 'operator approved through CLI',
        }),
      ],
    });
    if (submittedRun.state !== 'running') {
      throw new Error(`Workflow CLI smoke expected running after submit-input, got ${submittedRun.state ?? 'unknown'}.`);
    }
    if (submittedRun.intervention?.last_resolution?.action !== 'submit_input') {
      throw new Error('Workflow CLI smoke did not persist the submit-input resolution.');
    }

    const secondRequestId = crypto.randomUUID();
    await transitionRun(automationToken, options, manualRun.id, {
      state: 'awaiting_input',
      message: 'resume required',
      data: {
        intervention_request: {
          request_id: secondRequestId,
          kind: 'confirmation',
          prompt: 'Resume the CLI smoke',
        },
      },
    });

    const resumedRun = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'run',
        'resume',
        manualRun.id,
        '--body-json',
        JSON.stringify({
          comment: 'operator resumed through CLI',
        }),
      ],
    });
    if (resumedRun.state !== 'running') {
      throw new Error(`Workflow CLI smoke expected running after resume, got ${resumedRun.state ?? 'unknown'}.`);
    }
    if (resumedRun.intervention?.last_resolution?.action !== 'resume') {
      throw new Error('Workflow CLI smoke did not persist the resume resolution.');
    }

    const thirdRequestId = crypto.randomUUID();
    await transitionRun(automationToken, options, manualRun.id, {
      state: 'awaiting_input',
      message: 'reject required',
      data: {
        intervention_request: {
          request_id: thirdRequestId,
          kind: 'approval',
          prompt: 'Reject the CLI smoke',
        },
      },
    });

    const rejectedRun = runWorkflowCli({
      cwd: cliCwd,
      env: cliEnv,
      args: [
        'workflow',
        'run',
        'reject',
        manualRun.id,
        '--body-json',
        JSON.stringify({
          reason: 'operator denied through CLI',
        }),
      ],
    });
    if (rejectedRun.state !== 'failed') {
      throw new Error(`Workflow CLI smoke expected failed after reject, got ${rejectedRun.state ?? 'unknown'}.`);
    }
    if (rejectedRun.intervention?.last_resolution?.action !== 'reject') {
      throw new Error('Workflow CLI smoke did not persist the reject resolution.');
    }

    const summary = {
      workflowId: workflow.id,
      workflowVersion: version.version,
      workflowSourceCommit: version.source?.resolved_commit ?? null,
      runId: run.id,
      state: waitedRun.state,
      sessionId: waitedRun.session_id,
      producedFileId: producedFile.file_id,
      downloadedBytes: downloadSummary.byte_count,
      outputTitle: waitedRun.output?.title ?? null,
      interventionRunId: manualRun.id,
      interventionFinalState: rejectedRun.state,
    };

    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
      log(`Wrote summary to ${options.outputPath}`);
    }

    console.log(JSON.stringify(summary, null, 2));
  } finally {
    await context?.close().catch(() => {});
    await browser.close().catch(() => {});
    if (localWorkflowSource?.cleanup) {
      await localWorkflowSource.cleanup().catch(() => {});
    }
  }
}

main().catch((error) => {
  log(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});
