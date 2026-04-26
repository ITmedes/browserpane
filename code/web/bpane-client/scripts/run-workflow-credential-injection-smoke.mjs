import fs from 'node:fs/promises';
import process from 'node:process';

import { chromium } from 'playwright-core';

import {
  buildWorkflowWorkerImage,
  cleanupWorkflowSmokeSessions,
  configurePage,
  createLocalWorkflowRepo,
  createLogger,
  ensureLoggedIn,
  fetchJson,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

const log = createLogger('workflow-credential-injection-smoke');

function workflowCredentialInjectionEntrypoint() {
  return `export default async function run({
  page,
  credentialBindings,
  credentials,
  sessionId,
  workflowRunId,
  automationTaskId,
}) {
  const targetOrigin = 'http://web:8080';
  const byMode = Object.fromEntries(
    credentialBindings.map((binding) => [binding.injectionMode, binding]),
  );
  const formBinding = byMode.form_fill;
  const cookieBinding = byMode.cookie_seed;
  const storageBinding = byMode.storage_seed;
  const totpBinding = byMode.totp_fill;
  if (!formBinding || !cookieBinding || !storageBinding || !totpBinding) {
    throw new Error('Expected form_fill, cookie_seed, storage_seed, and totp_fill bindings');
  }

  await page.goto('http://web:8080/workflow-credential-fixture.html', { waitUntil: 'networkidle' });
  const storageSummary = await credentials.apply(storageBinding.id, { page, targetOrigin });
  const cookieSummary = await credentials.apply(cookieBinding.id, { page, targetOrigin });
  const formSummary = await credentials.apply(formBinding.id, { page, targetOrigin });
  const generatedTotp = await credentials.generateTotp(totpBinding.id, targetOrigin);
  const appliedTotp = await credentials.apply(totpBinding.id, { page, targetOrigin });
  const fixtureState = await page.evaluate(() => ({
    cookie: document.cookie,
    localValue: window.localStorage.getItem('bpane-local'),
    sessionValue: window.sessionStorage.getItem('bpane-session'),
    otpValue: document.getElementById('otp')?.value ?? '',
  }));

  if (fixtureState.otpValue !== generatedTotp.code || fixtureState.otpValue !== appliedTotp.code) {
    throw new Error('TOTP credential application did not fill the expected code');
  }

  console.log('workflow applied credential injection modes');
  await page.click('button[type="submit"]');
  await page.waitForFunction(() => document.title === 'Credential Fixture Authenticated');
  return {
    title: await page.title(),
    final_url: page.url(),
    cookie_value: fixtureState.cookie,
    local_storage_value: fixtureState.localValue,
    session_storage_value: fixtureState.sessionValue,
    otp_length: fixtureState.otpValue.length,
    cookie_count: cookieSummary.cookie_count ?? null,
    local_storage_count: storageSummary.local_storage_count ?? null,
    session_storage_count: storageSummary.session_storage_count ?? null,
    form_field_count: formSummary.field_count ?? null,
    session_id: sessionId,
    workflow_run_id: workflowRunId,
    automation_task_id: automationTaskId,
  };
}
`;
}

async function createCredentialBinding(accessToken, options, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/credential-bindings`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-credential-injection-smoke',
      description: 'Validate generic workflow credential injection helpers',
      labels: {
        suite: 'workflow-credential-injection-smoke',
      },
    }),
  });
}

async function createWorkflowVersion(accessToken, options, workflowId, source, bindingIds) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      version: 'v1',
      executor: 'playwright',
      entrypoint: 'workflows/credentials/run.mjs',
      source: {
        kind: 'git',
        repository_url: source.repositoryUrl,
        ref: 'refs/heads/main',
        root_path: 'workflows',
      },
      allowed_credential_binding_ids: bindingIds,
      default_session: {
        labels: {
          origin: 'workflow-credential-injection-smoke',
        },
      },
    }),
  });
}

async function createWorkflowRun(accessToken, options, workflowId, bindingIds) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      workflow_id: workflowId,
      version: 'v1',
      credential_binding_ids: bindingIds,
      labels: {
        suite: 'workflow-credential-injection-smoke',
      },
    }),
  });
}

async function fetchWorkflowRun(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchWorkflowRunLogs(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/logs`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function deleteSession(accessToken, options, sessionId) {
  const response = await fetch(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok && response.status !== 404) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

async function main() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-credential-injection-smoke.mjs');
  const browser = await launchChrome(chromium, options);

  let context = null;
  let page = null;
  let accessToken = '';
  let createdSessionId = '';
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
    localWorkflowSource = await createLocalWorkflowRepo(
      '.workflow-credential-injection-smoke-repo-',
      {
        'workflows/credentials/run.mjs': workflowCredentialInjectionEntrypoint(),
      },
    );

    log('Building workflow-worker image');
    buildWorkflowWorkerImage();

    const formBinding = await createCredentialBinding(accessToken, options, {
      name: 'demo-form-fill',
      provider: 'vault_kv_v2',
      namespace: 'smoke',
      allowed_origins: ['http://web:8080'],
      injection_mode: 'form_fill',
      secret_payload: {
        fields: [
          { selector: '#username', value: 'demo' },
          { selector: '#password', value: 'demo-demo' },
        ],
      },
      labels: {
        suite: 'workflow-credential-injection-smoke',
      },
    });
    const cookieBinding = await createCredentialBinding(accessToken, options, {
      name: 'demo-cookie-seed',
      provider: 'vault_kv_v2',
      namespace: 'smoke',
      allowed_origins: ['http://web:8080'],
      injection_mode: 'cookie_seed',
      secret_payload: {
        cookies: [{ name: 'bpane-cookie', value: 'cookie-demo' }],
      },
      labels: {
        suite: 'workflow-credential-injection-smoke',
      },
    });
    const storageBinding = await createCredentialBinding(accessToken, options, {
      name: 'demo-storage-seed',
      provider: 'vault_kv_v2',
      namespace: 'smoke',
      allowed_origins: ['http://web:8080'],
      injection_mode: 'storage_seed',
      secret_payload: {
        local_storage: {
          'bpane-local': 'local-demo',
        },
        session_storage: {
          'bpane-session': 'session-demo',
        },
      },
      labels: {
        suite: 'workflow-credential-injection-smoke',
      },
    });
    const totpBinding = await createCredentialBinding(accessToken, options, {
      name: 'demo-totp-fill',
      provider: 'vault_kv_v2',
      namespace: 'smoke',
      allowed_origins: ['http://web:8080'],
      injection_mode: 'totp_fill',
      totp: {
        issuer: 'BrowserPane',
        account_name: 'demo',
        period_sec: 30,
        digits: 6,
      },
      secret_payload: {
        secret: 'JBSWY3DPEHPK3PXP',
        selector: '#otp',
      },
      labels: {
        suite: 'workflow-credential-injection-smoke',
      },
    });
    const bindingIds = [formBinding.id, cookieBinding.id, storageBinding.id, totpBinding.id];
    if (bindingIds.some((entry) => !entry)) {
      throw new Error('Workflow credential injection smoke failed to create all credential bindings.');
    }

    const workflow = await createWorkflow(accessToken, options);
    const version = await createWorkflowVersion(
      accessToken,
      options,
      workflow.id,
      localWorkflowSource,
      bindingIds,
    );
    if (version.source?.resolved_commit !== localWorkflowSource.commit) {
      throw new Error('Workflow version did not pin the expected local git commit.');
    }

    const createdRun = await createWorkflowRun(accessToken, options, workflow.id, bindingIds);
    const runId = createdRun.id ?? '';
    createdSessionId = createdRun.session_id ?? '';
    if (!runId || !createdSessionId) {
      throw new Error('Workflow run creation did not return run and session ids.');
    }

    const succeededRun = await poll(
      'workflow credential injection success',
      () => fetchWorkflowRun(accessToken, options, runId),
      (run) => run?.state === 'succeeded',
      options.connectTimeoutMs,
    );
    if (succeededRun.output?.title !== 'Credential Fixture Authenticated') {
      throw new Error('Workflow credential injection run did not reach the authenticated fixture state.');
    }
    if (succeededRun.output?.cookie_value !== 'bpane-cookie=cookie-demo') {
      throw new Error('Workflow credential injection run did not seed the expected cookie.');
    }
    if (succeededRun.output?.local_storage_value !== 'local-demo') {
      throw new Error('Workflow credential injection run did not seed localStorage.');
    }
    if (succeededRun.output?.session_storage_value !== 'session-demo') {
      throw new Error('Workflow credential injection run did not seed sessionStorage.');
    }
    if (succeededRun.output?.otp_length !== 6) {
      throw new Error('Workflow credential injection run did not fill a 6-digit TOTP value.');
    }
    if (succeededRun.output?.form_field_count !== 2) {
      throw new Error('Workflow credential injection run did not fill the expected form fields.');
    }

    const logs = await fetchWorkflowRunLogs(accessToken, options, runId);
    if (
      !logs.logs.some(
        (entry) =>
          entry.source === 'automation_task' &&
          entry.message.includes('workflow applied credential injection modes'),
      )
    ) {
      throw new Error('Workflow credential injection logs are missing mode-application evidence.');
    }

    const summary = {
      workflowId: workflow.id,
      workflowVersion: 'v1',
      workflowSourceCommit: version.source?.resolved_commit ?? null,
      runId,
      state: succeededRun.state,
      sessionId: createdSessionId,
      automationTaskId: succeededRun.automation_task_id,
      outputTitle: succeededRun.output?.title ?? null,
      cookieValue: succeededRun.output?.cookie_value ?? null,
      localStorageValue: succeededRun.output?.local_storage_value ?? null,
      sessionStorageValue: succeededRun.output?.session_storage_value ?? null,
      otpLength: succeededRun.output?.otp_length ?? null,
      logs: logs.logs.length,
    };

    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
      log(`Wrote summary to ${options.outputPath}`);
    }

    console.log(JSON.stringify(summary, null, 2));
  } finally {
    if (createdSessionId && accessToken) {
      await deleteSession(accessToken, options, createdSessionId).catch(() => {});
    }
    if (localWorkflowSource?.cleanup) {
      await localWorkflowSource.cleanup().catch(() => {});
    }
    await context?.close().catch(() => {});
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(
    `[workflow-credential-injection-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
