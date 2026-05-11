import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export const DEFAULTS = {
  pageUrl: 'http://localhost:8080',
  certSpki: process.env.BPANE_BENCHMARK_CERT_SPKI ?? '',
  connectTimeoutMs: 30000,
  headless: false,
  outputPath: '',
};

export const TEST_EMBED_PATH = '/test-embed.html';

const COMMON_CHROME_PATHS = [
  process.env.BPANE_BENCHMARK_CHROME,
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
  '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
].filter(Boolean);

export const PROJECT_ROOT = fileURLToPath(new URL('../../../../', import.meta.url));

export function parseSmokeArgs(argv, usageLabel) {
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
      printHelp(usageLabel);
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return options;
}

function printHelp(usageLabel) {
  console.log(`
Usage: node scripts/${usageLabel} [options]

Options:
  --page-url <url>            Local web origin or test page URL (default: ${DEFAULTS.pageUrl})
  --cert-spki <base64>        SPKI pin for the local gateway cert
  --connect-timeout-ms <ms>   Connect timeout (default: ${DEFAULTS.connectTimeoutMs})
  --output <path>             Write JSON summary to file
  --headless                  Run headless
`);
}

export function createLogger(prefix) {
  return (message) => {
    console.log(`[${prefix}] ${message}`);
  };
}

export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function poll(description, fn, predicate, timeoutMs, intervalMs = 500) {
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

export async function resolveChromeExecutable() {
  for (const candidate of COMMON_CHROME_PATHS) {
    try {
      await fs.access(candidate);
      return candidate;
    } catch {
      // Ignore missing binaries.
    }
  }
  throw new Error(
    'No Chrome/Chromium executable found. Set BPANE_BENCHMARK_CHROME to a local Chrome path.',
  );
}

export async function resolveCertSpki(options) {
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

export function apiOrigin(options) {
  return new URL('/', options.pageUrl).origin;
}

export function testEmbedPageUrl(options) {
  const url = new URL(options.pageUrl);
  if (url.pathname === '/' || url.pathname === '') {
    url.pathname = TEST_EMBED_PATH;
  }
  return url.toString();
}

export async function fetchAuthConfig(options) {
  try {
    const response = await fetch(new URL('/auth-config.json', apiOrigin(options)));
    if (!response.ok) {
      return null;
    }
    return await response.json();
  } catch {
    return null;
  }
}

export async function configurePage(page, options) {
  await page.goto(testEmbedPageUrl(options), { waitUntil: 'networkidle' });
  await page.waitForFunction(() => Boolean(window.__bpaneAuth));
}

export async function ensureLoggedIn(page, options) {
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

  const pageUrl = new URL(testEmbedPageUrl(options));
  const targetPrefix = `${pageUrl.origin}${pageUrl.pathname}`;
  await page.waitForURL(new RegExp(`^${targetPrefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`), {
    timeout: options.connectTimeoutMs,
  });
  await page.waitForFunction(() => window.__bpaneAuth?.isAuthenticated?.() === true, {
    timeout: options.connectTimeoutMs,
  });
  return authConfig;
}

export async function getAccessToken(page) {
  return await page.evaluate(() => window.__bpaneAuth?.getAccessToken?.() ?? null);
}

export async function fetchJson(url, init) {
  const response = await fetch(url, init);
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  return await response.json();
}

export async function fetchBytes(url, init) {
  const response = await fetch(url, init);
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

export async function listSessions(accessToken, options) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

export async function deleteSession(accessToken, options, sessionId) {
  const response = await fetch(`${apiOrigin(options)}/api/v1/sessions/${sessionId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (response.ok || response.status === 404) {
    return;
  }
  if (response.status === 409) {
    const killResponse = await fetch(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/kill`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${accessToken}` },
    });
    if (killResponse.ok || killResponse.status === 404) {
      return;
    }
    const detail = await killResponse.text().catch(() => '');
    throw new Error(`HTTP ${killResponse.status}${detail ? ` ${detail}` : ''}`);
  }
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

export async function cleanupWorkflowSmokeSessions(accessToken, options, log = () => {}) {
  const response = await waitForWorkflowControlPlane(accessToken, options);
  const sessions = Array.isArray(response.sessions) ? response.sessions : [];
  let removed = 0;
  for (const session of sessions) {
    const sessionId = typeof session?.id === 'string' ? session.id : '';
    const sessionState = typeof session?.state === 'string' ? session.state : '';
    if (!sessionId) {
      continue;
    }
    if (sessionState === 'stopped') {
      continue;
    }
    await deleteSession(accessToken, options, sessionId);
    removed += 1;
  }
  if (removed > 0) {
    log(`Removed ${removed} stale visible sessions before the smoke run.`);
  }
}

export async function waitForWorkflowControlPlane(accessToken, options) {
  return await poll(
    'workflow control-plane readiness',
    async () => {
      try {
        return await listSessions(accessToken, options);
      } catch (error) {
        return error;
      }
    },
    (value) => !(value instanceof Error),
    Math.min(options.connectTimeoutMs, 15000),
    500,
  );
}

export function restartComposeService(service, { profile = null } = {}) {
  const args = ['compose', '-f', 'deploy/compose.yml'];
  if (profile) {
    args.push('--profile', profile);
  }
  args.push('restart', service);
  execFileSync('docker', args, {
    cwd: PROJECT_ROOT,
    stdio: 'inherit',
  });
}

export function recreateComposeServices(
  services,
  {
    profile = null,
    envOverrides = {},
  } = {},
) {
  const args = ['compose', '-f', 'deploy/compose.yml'];
  if (profile) {
    args.push('--profile', profile);
  }
  args.push('up', '-d', '--force-recreate', ...services);
  execFileSync('docker', args, {
    cwd: PROJECT_ROOT,
    stdio: 'inherit',
    env: {
      ...process.env,
      ...envOverrides,
    },
  });
}

export async function launchChrome(chromium, options) {
  const executablePath = await resolveChromeExecutable();
  const certSpki = await resolveCertSpki(options);
  const chromeArgs = [
    '--origin-to-force-quic-on=localhost:4433',
    '--disable-background-timer-throttling',
    '--disable-renderer-backgrounding',
    '--disable-backgrounding-occluded-windows',
  ];
  if (certSpki) {
    chromeArgs.push(`--ignore-certificate-errors-spki-list=${certSpki}`);
  }
  return await chromium.launch({
    headless: options.headless,
    executablePath,
    args: chromeArgs,
  });
}

export function runGitCommand(repoDir, args, options = {}) {
  return execFileSync('git', args, {
    cwd: repoDir,
    stdio: ['ignore', 'pipe', 'pipe'],
    encoding: 'utf8',
    ...options,
  });
}

export function initializeMainBranch(repoDir) {
  try {
    runGitCommand(repoDir, ['init', '-b', 'main']);
  } catch {
    runGitCommand(repoDir, ['init']);
    runGitCommand(repoDir, ['checkout', '-b', 'main']);
  }
}

export async function createLocalWorkflowRepo(prefix, files) {
  const repoDir = await fs.mkdtemp(path.join(PROJECT_ROOT, prefix));
  for (const [relativePath, content] of Object.entries(files)) {
    const absolutePath = path.join(repoDir, relativePath);
    await fs.mkdir(path.dirname(absolutePath), { recursive: true });
    await fs.writeFile(absolutePath, content, 'utf8');
  }
  initializeMainBranch(repoDir);
  runGitCommand(repoDir, ['config', 'user.name', 'BrowserPane Smoke']);
  runGitCommand(repoDir, ['config', 'user.email', 'smoke@browserpane.local']);
  runGitCommand(repoDir, ['add', '.']);
  runGitCommand(repoDir, ['commit', '-m', 'Add workflow smoke entrypoint']);
  const commit = runGitCommand(repoDir, ['rev-parse', 'HEAD']).trim();
  return {
    repoDir,
    repositoryUrl: `/workspace/${path.basename(repoDir)}`,
    commit,
    cleanup: async () => {
      await fs.rm(repoDir, { recursive: true, force: true });
    },
  };
}

export function buildWorkflowWorkerImage() {
  execFileSync(
    'docker',
    ['compose', '-f', 'deploy/compose.yml', '--profile', 'workflow', 'build', 'workflow-worker'],
    {
      cwd: PROJECT_ROOT,
      stdio: 'inherit',
    },
  );
}

export async function startWorkflowWebhookReceiver({
  containerNamePrefix = 'bpane-workflow-webhook',
  network = 'deploy_bpane-internal',
  port = 9107,
  statuses = [200],
} = {}) {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-workflow-webhook-'));
  const scriptPath = path.join(tempDir, 'receiver.py');
  const requestsPath = path.join(tempDir, 'requests.jsonl');
  const containerName = `${containerNamePrefix}-${process.pid}-${Date.now()}`;
  const script = `import http.server
import json
import os
import pathlib

statuses = [int(value) for value in os.environ.get("BPANE_WEBHOOK_STATUSES", "200").split(",") if value]
requests_path = pathlib.Path("/out/requests.jsonl")

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        body_text = self.rfile.read(length).decode("utf-8")
        try:
            body = json.loads(body_text)
        except json.JSONDecodeError:
            body = {"raw": body_text}
        entry = {
            "path": self.path,
            "headers": {key.lower(): value for key, value in self.headers.items()},
            "body": body,
        }
        requests_path.parent.mkdir(parents=True, exist_ok=True)
        with requests_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(entry) + "\\n")
        status = statuses.pop(0) if statuses else 200
        self.send_response(status)
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        self.wfile.write(b"ok")

    def log_message(self, format, *args):
        return

server = http.server.ThreadingHTTPServer(("0.0.0.0", ${port}), Handler)
server.serve_forever()
`;
  await fs.writeFile(scriptPath, script, 'utf8');

  execFileSync(
    'docker',
    [
      'run',
      '--rm',
      '-d',
      '--name',
      containerName,
      '--network',
      network,
      '-e',
      `BPANE_WEBHOOK_STATUSES=${statuses.join(',')}`,
      '-v',
      `${tempDir}:/out`,
      'python:3.12-alpine',
      'python',
      '/out/receiver.py',
    ],
    {
      cwd: PROJECT_ROOT,
      stdio: 'pipe',
    },
  );

  return {
    targetUrl: `http://${containerName}:${port}/events`,
    async readRequests() {
      try {
        const content = await fs.readFile(requestsPath, 'utf8');
        return content
          .split('\n')
          .filter(Boolean)
          .map((line) => JSON.parse(line));
      } catch (error) {
        if (error && typeof error === 'object' && error.code === 'ENOENT') {
          return [];
        }
        throw error;
      }
    },
    async cleanup() {
      try {
        execFileSync('docker', ['rm', '-f', containerName], {
          cwd: PROJECT_ROOT,
          stdio: 'pipe',
        });
      } catch {
        // Ignore already-stopped containers.
      }
      await fs.rm(tempDir, { recursive: true, force: true }).catch(() => {});
    },
  };
}

export async function inspectZipEntries(bytes) {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-workflow-source-'));
  const archivePath = path.join(tempDir, 'source.zip');
  try {
    await fs.writeFile(archivePath, bytes);
    const output = execFileSync(
      'python3',
      [
        '-c',
        'import json, sys, zipfile; archive = zipfile.ZipFile(sys.argv[1]); print(json.dumps(archive.namelist()))',
        archivePath,
      ],
      { encoding: 'utf8' },
    );
    return JSON.parse(output);
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true }).catch(() => {});
  }
}
