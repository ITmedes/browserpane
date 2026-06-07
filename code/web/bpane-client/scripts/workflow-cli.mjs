#!/usr/bin/env node

import fs from 'node:fs/promises';
import process from 'node:process';

function printUsage() {
  const lines = [
    'Usage:',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow list',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow create --body-json <json>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow get <workflow-id>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow version create <workflow-id> --body-json <json>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow version get <workflow-id> <version>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run create --body-json <json>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run create --workflow-id <id> [--version <version>] [--project-id <id>] [--create-session|--session-id <id>] [--input-json <json>] [--label key=value] [--summary]',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run get <run-id>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run wait <run-id> [--target-state <state>]',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run logs <run-id>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run events <run-id>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run submit-input <run-id> --body-json <json>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run resume <run-id> --body-json <json>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run reject <run-id> --body-json <json>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run cancel <run-id>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run produced-files <run-id>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run download-produced-file <run-id> <file-id> --output <path>',
    '',
    'Options:',
    '  --access-token <token>   Bearer token. Falls back to BPANE_ACCESS_TOKEN.',
    '  --api-url <url>          Gateway base URL. Falls back to BPANE_API_URL or http://localhost:8932.',
    '  --body-json <json>       Inline JSON request body.',
    '  --body-file <path>       JSON request body file.',
    '  --workflow-id <id>       Workflow definition id for ergonomic run create.',
    '  --version <version>      Workflow version for ergonomic run create. Default v1.',
    '  --project-id <id>        Project id for ergonomic run create.',
    '  --session-id <id>        Existing session id for ergonomic run create.',
    '  --create-session         Create a new session for ergonomic run create.',
    '  --input-json <json>      Inline workflow run input object.',
    '  --input-file <path>      Workflow run input object file.',
    '  --label key=value        Add a workflow run label. Repeatable.',
    '  --source-system <value>  External source system for the run.',
    '  --source-reference <ref> External source reference for the run.',
    '  --client-request-id <id> Stable idempotency key for run create.',
    '  --summary                Print compact workflow-run summary JSON.',
    '  --output <path>          Output file path for download-produced-file.',
    '  --timeout-ms <ms>        Wait timeout. Default 60000.',
    '  --interval-ms <ms>       Wait poll interval. Default 1000.',
    '  --target-state <state>   Required state for workflow run wait.',
  ];
  console.error(lines.join('\n'));
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

function parseArgs(argv) {
  const positionals = [];
  const options = new Map();
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith('--')) {
      positionals.push(token);
      continue;
    }

    const [rawName, inlineValue] = token.split('=', 2);
    const name = rawName.slice(2);
    if (!name) {
      fail('Encountered an empty option name.');
    }
    const next = argv[index + 1];
    const value =
      inlineValue !== undefined
        ? inlineValue
        : next !== undefined && !next.startsWith('--')
          ? (index += 1, next)
          : 'true';
    const existing = options.get(name);
    if (existing) {
      existing.push(value);
    } else {
      options.set(name, [value]);
    }
  }
  return { positionals, options };
}

function getOption(options, name, fallback = null) {
  const values = options.get(name);
  if (!values || !values.length) {
    return fallback;
  }
  return values[values.length - 1];
}

function getOptions(options, name) {
  return options.get(name) ?? [];
}

function hasOption(options, name) {
  return options.has(name);
}

function requireOption(options, name) {
  const value = getOption(options, name, null);
  if (value === null || value === '') {
    fail(`Missing required option --${name}.`);
  }
  return value;
}

function normalizeApiUrl(value) {
  return value.replace(/\/+$/u, '');
}

function parseJsonOption(options, jsonName, fileName, requiredLabel = null) {
  const inlineJson = getOption(options, jsonName, null);
  const filePath = getOption(options, fileName, null);
  if (inlineJson && filePath) {
    fail(`Use only one of --${jsonName} or --${fileName}.`);
  }
  if (!inlineJson && !filePath) {
    if (requiredLabel) {
      fail(`Missing ${requiredLabel}. Pass --${jsonName} or --${fileName}.`);
    }
    return undefined;
  }
  return { inlineJson, filePath };
}

async function readJsonOption(options, jsonName, fileName, requiredLabel = null) {
  const source = parseJsonOption(options, jsonName, fileName, requiredLabel);
  if (!source) {
    return undefined;
  }
  try {
    if (source.inlineJson) {
      return JSON.parse(source.inlineJson);
    }
    return JSON.parse(await fs.readFile(source.filePath, 'utf8'));
  } catch (error) {
    fail(`Failed to parse --${source.inlineJson ? jsonName : fileName}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

async function readJsonBody(options) {
  return await readJsonOption(options, 'body-json', 'body-file', 'request body JSON');
}

function parseLabelMap(options) {
  const labels = {};
  for (const label of getOptions(options, 'label')) {
    const separator = label.indexOf('=');
    if (separator <= 0) {
      fail(`Invalid --label value ${label}. Expected key=value.`);
    }
    const key = label.slice(0, separator).trim();
    const value = label.slice(separator + 1);
    if (!key) {
      fail(`Invalid --label value ${label}. Label key must not be empty.`);
    }
    labels[key] = value;
  }
  return Object.keys(labels).length ? labels : undefined;
}

async function buildWorkflowRunCreateBody(options) {
  const hasRawBody = hasOption(options, 'body-json') || hasOption(options, 'body-file');
  const body = hasRawBody ? await readJsonBody(options) : {};
  if (!body || typeof body !== 'object' || Array.isArray(body)) {
    fail('Workflow run create body must be a JSON object.');
  }

  const workflowId = getOption(options, 'workflow-id', null);
  if (workflowId) {
    body.workflow_id = workflowId;
  }
  if (!body.workflow_id) {
    fail('workflow run create requires --workflow-id or body.workflow_id.');
  }

  const version = getOption(options, 'version', null);
  if (version) {
    body.version = version;
  } else if (!body.version) {
    body.version = 'v1';
  }

  const projectId = getOption(options, 'project-id', null);
  if (projectId) {
    body.project_id = projectId;
  }

  const sourceSystem = getOption(options, 'source-system', null);
  if (sourceSystem) {
    body.source_system = sourceSystem;
  }
  const sourceReference = getOption(options, 'source-reference', null);
  if (sourceReference) {
    body.source_reference = sourceReference;
  }
  const clientRequestId = getOption(options, 'client-request-id', null);
  if (clientRequestId) {
    body.client_request_id = clientRequestId;
  }

  const input = await readJsonOption(options, 'input-json', 'input-file');
  if (input !== undefined) {
    body.input = input;
  }

  const labels = parseLabelMap(options);
  if (labels) {
    body.labels = {
      ...(body.labels && typeof body.labels === 'object' && !Array.isArray(body.labels)
        ? body.labels
        : {}),
      ...labels,
    };
  }

  const sessionId = getOption(options, 'session-id', null);
  const createSession = hasOption(options, 'create-session');
  if (sessionId && createSession) {
    fail('Use only one of --session-id or --create-session.');
  }
  if (sessionId) {
    body.session = { existing_session_id: sessionId };
  } else if (createSession) {
    const createSessionBody =
      body.session && typeof body.session === 'object' && !Array.isArray(body.session) && body.session.create_session
        ? body.session.create_session
        : {};
    body.session = {
      create_session: {
        ...(typeof createSessionBody === 'object' && !Array.isArray(createSessionBody)
          ? createSessionBody
          : {}),
      },
    };
    if (projectId && body.session.create_session.project_id === undefined) {
      body.session.create_session.project_id = projectId;
    }
  }

  return body;
}

function buildHeaders(accessToken, extraHeaders = {}) {
  return {
    Authorization: `Bearer ${accessToken}`,
    ...extraHeaders,
  };
}

async function requestJson(baseUrl, accessToken, path, init = {}) {
  const headers = buildHeaders(accessToken, init.headers);
  if (init.body !== undefined && !headers['Content-Type']) {
    headers['Content-Type'] = 'application/json';
  }
  const response = await fetch(`${baseUrl}${path}`, {
    ...init,
    headers,
  });
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    fail(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  if (response.status === 204) {
    return null;
  }
  return await response.json();
}

async function requestBytes(baseUrl, accessToken, path) {
  const response = await fetch(`${baseUrl}${path}`, {
    headers: buildHeaders(accessToken),
  });
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    fail(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

async function waitForRun(baseUrl, accessToken, runId, options) {
  const timeoutMs = Number.parseInt(getOption(options, 'timeout-ms', '60000'), 10);
  const intervalMs = Number.parseInt(getOption(options, 'interval-ms', '1000'), 10);
  const targetState = getOption(options, 'target-state', null);
  const deadline = Date.now() + timeoutMs;

  while (Date.now() <= deadline) {
    const run = await requestJson(baseUrl, accessToken, `/api/v1/workflow-runs/${encodeURIComponent(runId)}`);
    const state = typeof run?.state === 'string' ? run.state : '';
    const terminal = ['succeeded', 'failed', 'cancelled', 'timed_out'].includes(state);
    if (targetState) {
      if (state === targetState) {
        return run;
      }
      if (terminal && state !== targetState) {
        fail(`Workflow run ${runId} reached terminal state ${state} before ${targetState}.`);
      }
    } else if (terminal) {
      return run;
    }
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }

  fail(`Timed out waiting for workflow run ${runId}.`);
}

function printJson(value) {
  console.log(JSON.stringify(value, null, 2));
}

function projectLabel(run) {
  if (run?.project && typeof run.project.name === 'string' && run.project.name.trim()) {
    return run.project.name;
  }
  return run?.project_id ?? null;
}

function workflowRunSummary(run) {
  return {
    id: run?.id ?? null,
    state: run?.state ?? null,
    workflow_id: run?.workflow_id ?? run?.workflow_definition_id ?? null,
    workflow_version: run?.workflow_version ?? null,
    session_id: run?.session_id ?? null,
    project_id: run?.project_id ?? null,
    project: projectLabel(run),
    project_admission_state: run?.project_admission?.state ?? null,
    project_admission_reason_code: run?.project_admission?.reason_code ?? null,
    admission_reason: run?.admission?.reason ?? null,
    client_request_id: run?.client_request_id ?? null,
    source_system: run?.source_system ?? null,
    source_reference: run?.source_reference ?? null,
  };
}

function printWorkflowRun(value, options) {
  printJson(hasOption(options, 'summary') ? workflowRunSummary(value) : value);
}

async function main() {
  const { positionals, options } = parseArgs(process.argv.slice(2));
  const wantsHelp = getOption(options, 'help', null) === 'true';
  if (!positionals.length || wantsHelp) {
    printUsage();
    process.exit(wantsHelp ? 0 : 1);
  }

  const accessToken =
    getOption(options, 'access-token', null) ?? process.env.BPANE_ACCESS_TOKEN ?? null;
  if (!accessToken) {
    fail('Missing bearer token. Pass --access-token or set BPANE_ACCESS_TOKEN.');
  }
  const baseUrl = normalizeApiUrl(
    getOption(options, 'api-url', null) ?? process.env.BPANE_API_URL ?? 'http://localhost:8932',
  );

  const [scope, subScope, action, extra] = positionals;
  if (scope !== 'workflow') {
    printUsage();
    fail(`Unknown top-level scope ${scope}.`);
  }

  if (subScope === 'list' && !action) {
    printJson(await requestJson(baseUrl, accessToken, '/api/v1/workflows'));
    return;
  }

  if (subScope === 'create' && !action) {
    const body = await readJsonBody(options);
    printJson(
      await requestJson(baseUrl, accessToken, '/api/v1/workflows', {
        method: 'POST',
        body: JSON.stringify(body),
      }),
    );
    return;
  }

  if (subScope === 'get' && action && !extra) {
    printJson(
      await requestJson(
        baseUrl,
        accessToken,
        `/api/v1/workflows/${encodeURIComponent(action)}`,
      ),
    );
    return;
  }

  if (subScope === 'version') {
    if (action === 'create') {
      const workflowId = extra;
      if (!workflowId) {
        fail('Usage: workflow version create <workflow-id> --body-json <json>');
      }
      const body = await readJsonBody(options);
      printJson(
        await requestJson(
          baseUrl,
          accessToken,
          `/api/v1/workflows/${encodeURIComponent(workflowId)}/versions`,
          {
            method: 'POST',
            body: JSON.stringify(body),
          },
        ),
      );
      return;
    }

    if (action === 'get') {
      const workflowId = extra;
      const version = positionals[4];
      if (!workflowId || !version) {
        fail('Usage: workflow version get <workflow-id> <version>');
      }
      printJson(
        await requestJson(
          baseUrl,
          accessToken,
          `/api/v1/workflows/${encodeURIComponent(workflowId)}/versions/${encodeURIComponent(version)}`,
        ),
      );
      return;
    }
  }

  if (subScope === 'run') {
    if (action === 'create' && !extra) {
      const body = await buildWorkflowRunCreateBody(options);
      printWorkflowRun(
        await requestJson(baseUrl, accessToken, '/api/v1/workflow-runs', {
          method: 'POST',
          body: JSON.stringify(body),
        }),
        options,
      );
      return;
    }

    const runId = extra;
    if (!runId) {
      fail(`Usage: workflow run ${action ?? '<action>'} <run-id>`);
    }

    if (action === 'get') {
      printWorkflowRun(
        await requestJson(
          baseUrl,
          accessToken,
          `/api/v1/workflow-runs/${encodeURIComponent(runId)}`,
        ),
        options,
      );
      return;
    }

    if (action === 'wait') {
      printWorkflowRun(await waitForRun(baseUrl, accessToken, runId, options), options);
      return;
    }

    if (action === 'logs') {
      printJson(
        await requestJson(
          baseUrl,
          accessToken,
          `/api/v1/workflow-runs/${encodeURIComponent(runId)}/logs`,
        ),
      );
      return;
    }

    if (action === 'events') {
      printJson(
        await requestJson(
          baseUrl,
          accessToken,
          `/api/v1/workflow-runs/${encodeURIComponent(runId)}/events`,
        ),
      );
      return;
    }

    if (action === 'submit-input') {
      const body = await readJsonBody(options);
      printJson(
        await requestJson(
          baseUrl,
          accessToken,
          `/api/v1/workflow-runs/${encodeURIComponent(runId)}/submit-input`,
          {
            method: 'POST',
            body: JSON.stringify(body),
          },
        ),
      );
      return;
    }

    if (action === 'resume') {
      const body = await readJsonBody(options);
      printJson(
        await requestJson(
          baseUrl,
          accessToken,
          `/api/v1/workflow-runs/${encodeURIComponent(runId)}/resume`,
          {
            method: 'POST',
            body: JSON.stringify(body),
          },
        ),
      );
      return;
    }

    if (action === 'reject') {
      const body = await readJsonBody(options);
      printJson(
        await requestJson(
          baseUrl,
          accessToken,
          `/api/v1/workflow-runs/${encodeURIComponent(runId)}/reject`,
          {
            method: 'POST',
            body: JSON.stringify(body),
          },
        ),
      );
      return;
    }

    if (action === 'cancel') {
      printJson(
        await requestJson(
          baseUrl,
          accessToken,
          `/api/v1/workflow-runs/${encodeURIComponent(runId)}/cancel`,
          { method: 'POST' },
        ),
      );
      return;
    }

    if (action === 'produced-files') {
      printJson(
        await requestJson(
          baseUrl,
          accessToken,
          `/api/v1/workflow-runs/${encodeURIComponent(runId)}/produced-files`,
        ),
      );
      return;
    }

    if (action === 'download-produced-file') {
      const fileId = positionals[4];
      if (!fileId) {
        fail('Usage: workflow run download-produced-file <run-id> <file-id> --output <path>');
      }
      const outputPath = requireOption(options, 'output');
      const bytes = await requestBytes(
        baseUrl,
        accessToken,
        `/api/v1/workflow-runs/${encodeURIComponent(runId)}/produced-files/${encodeURIComponent(fileId)}/content`,
      );
      await fs.writeFile(outputPath, bytes);
      printJson({
        run_id: runId,
        file_id: fileId,
        output_path: outputPath,
        byte_count: bytes.length,
      });
      return;
    }
  }

  printUsage();
  fail(`Unknown workflow command: ${positionals.join(' ')}`);
}

await main();
