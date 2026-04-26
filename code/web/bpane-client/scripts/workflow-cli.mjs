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
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run get <run-id>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run wait <run-id> [--target-state <state>]',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run logs <run-id>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run events <run-id>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run cancel <run-id>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run produced-files <run-id>',
    '  node scripts/workflow-cli.mjs [--api-url URL] workflow run download-produced-file <run-id> <file-id> --output <path>',
    '',
    'Options:',
    '  --access-token <token>   Bearer token. Falls back to BPANE_ACCESS_TOKEN.',
    '  --api-url <url>          Gateway base URL. Falls back to BPANE_API_URL or http://localhost:8932.',
    '  --body-json <json>       Inline JSON request body.',
    '  --body-file <path>       JSON request body file.',
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

async function readJsonBody(options) {
  const bodyJson = getOption(options, 'body-json', null);
  const bodyFile = getOption(options, 'body-file', null);
  if (bodyJson && bodyFile) {
    fail('Use only one of --body-json or --body-file.');
  }
  if (!bodyJson && !bodyFile) {
    fail('This command requires --body-json or --body-file.');
  }
  try {
    if (bodyJson) {
      return JSON.parse(bodyJson);
    }
    return JSON.parse(await fs.readFile(bodyFile, 'utf8'));
  } catch (error) {
    fail(`Failed to parse request body JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
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
      const body = await readJsonBody(options);
      printJson(
        await requestJson(baseUrl, accessToken, '/api/v1/workflow-runs', {
          method: 'POST',
          body: JSON.stringify(body),
        }),
      );
      return;
    }

    const runId = extra;
    if (!runId) {
      fail(`Usage: workflow run ${action ?? '<action>'} <run-id>`);
    }

    if (action === 'get') {
      printJson(
        await requestJson(
          baseUrl,
          accessToken,
          `/api/v1/workflow-runs/${encodeURIComponent(runId)}`,
        ),
      );
      return;
    }

    if (action === 'wait') {
      printJson(await waitForRun(baseUrl, accessToken, runId, options));
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
