import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';

const DEFAULTS = {
  pageUrl: 'http://localhost:8080',
  certSpki: process.env.BPANE_BENCHMARK_CERT_SPKI ?? '',
  connectTimeoutMs: 30000,
  headless: false,
  outputPath: '',
};

const COMMON_CHROME_PATHS = [
  process.env.BPANE_BENCHMARK_CHROME,
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
  '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
].filter(Boolean);

function parseArgs(argv) {
  const options = { ...DEFAULTS };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const next = argv[i + 1];
    if (arg === '--page-url' && next) {
      options.pageUrl = next;
      i++;
    } else if (arg === '--cert-spki' && next) {
      options.certSpki = next;
      i++;
    } else if (arg === '--connect-timeout-ms' && next) {
      options.connectTimeoutMs = Number(next);
      i++;
    } else if (arg === '--output' && next) {
      options.outputPath = next;
      i++;
    } else if (arg === '--headless') {
      options.headless = true;
    } else if (arg === '--help') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return options;
}

function printHelp() {
  console.log(`
Usage: node scripts/run-file-workspace-smoke.mjs [options]

Options:
  --page-url <url>            Local test page URL (default: ${DEFAULTS.pageUrl})
  --cert-spki <base64>        SPKI pin for the local gateway cert
  --connect-timeout-ms <ms>   Connect timeout (default: ${DEFAULTS.connectTimeoutMs})
  --output <path>             Write JSON summary to file
  --headless                  Run headless
`);
}

function log(message) {
  console.log(`[file-workspace-smoke] ${message}`);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function poll(description, fn, predicate, timeoutMs, intervalMs = 500) {
  const startedAt = Date.now();
  let lastValue = null;
  while (Date.now() - startedAt < timeoutMs) {
    lastValue = await fn();
    if (predicate(lastValue)) {
      return lastValue;
    }
    await sleep(intervalMs);
  }
  throw new Error(`Timed out waiting for ${description}`);
}

async function resolveChromeExecutable() {
  for (const candidate of COMMON_CHROME_PATHS) {
    try {
      await fs.access(candidate);
      return candidate;
    } catch {
      // ignore
    }
  }
  throw new Error(
    'No Chrome/Chromium executable found. Set BPANE_BENCHMARK_CHROME to a local Chrome path.',
  );
}

async function resolveCertSpki(options) {
  if (options.certSpki?.trim()) {
    return options.certSpki.trim();
  }
  try {
    const value = await fs.readFile(
      new URL('../../../../dev/certs/cert-fingerprint.txt', import.meta.url),
      'utf8',
    );
    return value.trim();
  } catch {
    return '';
  }
}

async function fetchAuthConfig(options) {
  try {
    const response = await fetch(new URL('/auth-config.json', options.pageUrl));
    if (!response.ok) {
      return null;
    }
    return await response.json();
  } catch {
    return null;
  }
}

async function configurePage(page, options) {
  await page.goto(options.pageUrl, { waitUntil: 'networkidle' });
  await page.waitForFunction(() => Boolean(window.__bpaneAuth));
}

async function ensureLoggedIn(page, options) {
  const authConfig = await fetchAuthConfig(options);
  if (!authConfig || authConfig.mode !== 'oidc') {
    return authConfig;
  }

  const state = await page.evaluate(() => ({
    configured: window.__bpaneAuth?.isConfigured?.() ?? false,
    authenticated: window.__bpaneAuth?.isAuthenticated?.() ?? false,
    exampleUser: window.__bpaneAuth?.getExampleUser?.() ?? null,
  }));
  if (!state.configured || state.authenticated) {
    return authConfig;
  }

  const exampleUser = state.exampleUser;
  if (!exampleUser?.username || !exampleUser?.password) {
    throw new Error('OIDC auth is enabled, but no example user is configured for smoke login.');
  }

  await page.click('#btn-login');
  const username = page.locator('input[name="username"], #username').first();
  const password = page.locator('input[name="password"], #password').first();
  const loginState = await poll(
    'OIDC login readiness',
    async () => ({
      authenticated: await page
        .evaluate(() => window.__bpaneAuth?.isAuthenticated?.() === true)
        .catch(() => false),
      usernameVisible: await username.isVisible().catch(() => false),
    }),
    (value) => value.authenticated || value.usernameVisible,
    options.connectTimeoutMs,
  );
  if (!loginState.authenticated) {
    await username.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    await password.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    await username.fill(exampleUser.username);
    await password.fill(exampleUser.password);
    await page.locator('input[type="submit"], #kc-login').click();
  }

  const pageUrl = new URL(options.pageUrl);
  const targetPrefix = `${pageUrl.origin}${pageUrl.pathname}`;
  await page.waitForURL(new RegExp(`^${targetPrefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`), {
    timeout: options.connectTimeoutMs,
  });
  await page.waitForFunction(() => window.__bpaneAuth?.isAuthenticated?.() === true, {
    timeout: options.connectTimeoutMs,
  });
  return authConfig;
}

async function getAccessToken(page) {
  return await page.evaluate(() => window.__bpaneAuth?.getAccessToken?.() ?? null);
}

function apiUrl(path, options) {
  return new URL(path, options.pageUrl).toString();
}

async function fetchJson(url, init) {
  const response = await fetch(url, init);
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  return await response.json();
}

async function fetchBytes(url, init) {
  const response = await fetch(url, init);
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  return {
    headers: response.headers,
    bytes: Buffer.from(await response.arrayBuffer()),
  };
}

function sha256Hex(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const executablePath = await resolveChromeExecutable();
  const certSpki = await resolveCertSpki(options);

  log(`Launching Chromium from ${executablePath}`);
  const browser = await chromium.launch({
    executablePath,
    headless: options.headless,
    args: certSpki ? [`--ignore-certificate-errors-spki-list=${certSpki}`] : [],
  });

  try {
    const page = await browser.newPage();
    await configurePage(page, options);
    await ensureLoggedIn(page, options);
    const accessToken = await getAccessToken(page);
    if (!accessToken) {
      throw new Error('No access token available after smoke login.');
    }

    log('Creating file workspace');
    const workspace = await fetchJson(apiUrl('/api/v1/file-workspaces', options), {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${accessToken}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        name: 'workflow-smoke-workspace',
        description: 'Validate file workspace CRUD and content delivery',
        labels: {
          suite: 'smoke',
        },
      }),
    });

    const listedWorkspaces = await fetchJson(apiUrl('/api/v1/file-workspaces', options), {
      headers: {
        Authorization: `Bearer ${accessToken}`,
      },
    });
    if (!listedWorkspaces.workspaces.some((entry) => entry.id === workspace.id)) {
      throw new Error('Created workspace did not appear in owner-scoped workspace list.');
    }

    const fileBytes = Buffer.from('month,total\n2026-03,42\n', 'utf8');
    const provenance = {
      source_kind: 'git_materialized',
      repo_path: 'workflows/reporting/monthly.csv',
      commit: 'abc123def456',
    };

    log('Uploading workspace file');
    const uploadedFile = await fetchJson(
      apiUrl(`/api/v1/file-workspaces/${workspace.id}/files`, options),
      {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${accessToken}`,
          'Content-Type': 'text/csv',
          'x-bpane-file-name': 'monthly-report.csv',
          'x-bpane-file-provenance': JSON.stringify(provenance),
        },
        body: fileBytes,
      },
    );

    if (uploadedFile.sha256_hex !== sha256Hex(fileBytes)) {
      throw new Error('Uploaded file sha256_hex does not match local payload digest.');
    }
    if (uploadedFile.byte_count !== fileBytes.length) {
      throw new Error('Uploaded file byte_count does not match local payload length.');
    }

    const listedFiles = await fetchJson(
      apiUrl(`/api/v1/file-workspaces/${workspace.id}/files`, options),
      {
        headers: {
          Authorization: `Bearer ${accessToken}`,
        },
      },
    );
    if (!listedFiles.files.some((entry) => entry.id === uploadedFile.id)) {
      throw new Error('Uploaded file did not appear in workspace file list.');
    }

    const fetchedFile = await fetchJson(
      apiUrl(`/api/v1/file-workspaces/${workspace.id}/files/${uploadedFile.id}`, options),
      {
        headers: {
          Authorization: `Bearer ${accessToken}`,
        },
      },
    );
    if (
      fetchedFile.provenance?.source_kind !== provenance.source_kind ||
      fetchedFile.provenance?.repo_path !== provenance.repo_path ||
      fetchedFile.provenance?.commit !== provenance.commit
    ) {
      throw new Error('Uploaded file provenance did not round-trip through metadata fetch.');
    }

    log('Downloading workspace file content');
    const downloaded = await fetchBytes(
      apiUrl(`/api/v1/file-workspaces/${workspace.id}/files/${uploadedFile.id}/content`, options),
      {
        headers: {
          Authorization: `Bearer ${accessToken}`,
        },
      },
    );
    if (downloaded.headers.get('content-type') !== 'text/csv') {
      throw new Error(`Unexpected file content-type: ${downloaded.headers.get('content-type')}`);
    }
    if (!downloaded.bytes.equals(fileBytes)) {
      throw new Error('Downloaded workspace file bytes do not match uploaded payload.');
    }

    log('Deleting workspace file');
    await fetchJson(
      apiUrl(`/api/v1/file-workspaces/${workspace.id}/files/${uploadedFile.id}`, options),
      {
        method: 'DELETE',
        headers: {
          Authorization: `Bearer ${accessToken}`,
        },
      },
    );

    const finalList = await fetchJson(
      apiUrl(`/api/v1/file-workspaces/${workspace.id}/files`, options),
      {
        headers: {
          Authorization: `Bearer ${accessToken}`,
        },
      },
    );
    if (finalList.files.length !== 0) {
      throw new Error('Workspace file list should be empty after deleting the uploaded file.');
    }

    const summary = {
      workspaceId: workspace.id,
      uploadedFileId: uploadedFile.id,
      uploadedBytes: uploadedFile.byte_count,
      downloadedBytes: downloaded.bytes.length,
      sha256Hex: uploadedFile.sha256_hex,
    };

    if (options.outputPath) {
      await fs.writeFile(options.outputPath, JSON.stringify(summary, null, 2));
    }

    console.log(JSON.stringify(summary, null, 2));
    log('Smoke completed successfully');
  } finally {
    await browser.close();
  }
}

main().catch((error) => {
  console.error(`[file-workspace-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
