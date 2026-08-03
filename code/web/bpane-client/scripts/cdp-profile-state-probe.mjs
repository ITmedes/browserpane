import process from 'node:process';
import { lookup } from 'node:dns/promises';
import net from 'node:net';
import { pathToFileURL } from 'node:url';

const DEFAULTS = {
  action: 'get',
  cdpEndpoint: '',
  originUrl: 'http://web:8080/test-embed.html',
  key: 'bpane_context_probe',
  value: '',
  cookieName: 'bpane_context_probe',
  cookieValue: '',
  timeoutMs: 10000,
};

function parseArgs(argv) {
  const options = { ...DEFAULTS };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const next = argv[i + 1];
    if (arg === '--action' && next) {
      options.action = next;
      i++;
    } else if (arg === '--cdp-endpoint' && next) {
      options.cdpEndpoint = next;
      i++;
    } else if (arg === '--origin-url' && next) {
      options.originUrl = next;
      i++;
    } else if (arg === '--key' && next) {
      options.key = next;
      i++;
    } else if (arg === '--value' && next) {
      options.value = next;
      i++;
    } else if (arg === '--cookie-name' && next) {
      options.cookieName = next;
      i++;
    } else if (arg === '--cookie-value' && next) {
      options.cookieValue = next;
      i++;
    } else if (arg === '--timeout-ms' && next) {
      options.timeoutMs = Number(next);
      i++;
    } else if (arg === '--help') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  if (!options.cdpEndpoint) {
    throw new Error('--cdp-endpoint is required');
  }
  if (!['get', 'set'].includes(options.action)) {
    throw new Error('--action must be set or get');
  }
  return options;
}

function printHelp() {
  console.log(`
Usage: node scripts/cdp-profile-state-probe.mjs --cdp-endpoint <url> [options]

Options:
  --action <set|get>     Write or read profile-backed browser state
  --cdp-endpoint <url>   Chromium DevTools endpoint, for example http://host:9223
  --origin-url <url>     Origin to use for localStorage/cookie state
  --key <key>            localStorage key
  --value <value>        localStorage value for set
  --cookie-name <name>   Cookie name
  --cookie-value <value> Cookie value for set
  --timeout-ms <ms>      Probe timeout
  --help                Show this help
`);
}

function withTimeout(promise, timeoutMs, description) {
  let timeoutId;
  const timeout = new Promise((_, reject) => {
    timeoutId = setTimeout(() => reject(new Error(`Timed out waiting for ${description}`)), timeoutMs);
  });
  return Promise.race([promise, timeout]).finally(() => {
    clearTimeout(timeoutId);
  });
}

function sleep(durationMs) {
  return new Promise((resolve) => setTimeout(resolve, durationMs));
}

export function isTransientDocumentError(error) {
  const message = error instanceof Error ? error.message : String(error);
  return [
    'Inspected target navigated or closed',
    'Execution context was destroyed',
    'Cannot find context with specified id',
  ].some((fragment) => message.includes(fragment));
}

export function isExpectedDocumentReady(documentState, expectedUrl) {
  if (!documentState || !['interactive', 'complete'].includes(documentState.readyState)) {
    return false;
  }
  try {
    const actual = new URL(documentState.href);
    const expected = new URL(expectedUrl);
    actual.hash = '';
    expected.hash = '';
    return actual.href === expected.href;
  } catch {
    return false;
  }
}

async function evaluateValue(connection, expression) {
  const evaluation = await connection.send('Runtime.evaluate', {
    expression,
    returnByValue: true,
  });
  if (evaluation.exceptionDetails) {
    throw new Error(
      evaluation.exceptionDetails.exception?.description
      ?? evaluation.exceptionDetails.text
      ?? 'CDP evaluation failed with an unknown exception',
    );
  }
  return evaluation?.result?.value;
}

export async function waitForExpectedDocument(
  connection,
  expectedUrl,
  timeoutMs,
  { now = Date.now, wait = sleep, pollIntervalMs = 100 } = {},
) {
  const deadline = now() + timeoutMs;
  let lastDetail = 'no document state observed';
  let stableHref = '';
  let stablePasses = 0;

  while (now() <= deadline) {
    try {
      const documentState = await evaluateValue(
        connection,
        '({ href: location.href, readyState: document.readyState })',
      );
      lastDetail = JSON.stringify(documentState);
      if (isExpectedDocumentReady(documentState, expectedUrl)) {
        if (documentState.href === stableHref) {
          stablePasses++;
        } else {
          stableHref = documentState.href;
          stablePasses = 1;
        }
        if (stablePasses >= 2) {
          return documentState;
        }
      } else {
        stableHref = '';
        stablePasses = 0;
      }
    } catch (error) {
      if (!isTransientDocumentError(error)) {
        throw error;
      }
      lastDetail = error instanceof Error ? error.message : String(error);
      stableHref = '';
      stablePasses = 0;
    }

    const remainingMs = deadline - now();
    if (remainingMs <= 0) {
      break;
    }
    await wait(Math.min(pollIntervalMs, remainingMs));
  }

  throw new Error(`Timed out waiting for ${expectedUrl} to become stable; last state: ${lastDetail}`);
}

async function normalizeCdpEndpoint(cdpEndpoint) {
  const url = new URL(cdpEndpoint);
  if (url.hostname !== 'localhost' && net.isIP(url.hostname) === 0) {
    const resolved = await lookup(url.hostname);
    url.hostname = resolved.address;
  }
  return url.toString().replace(/\/$/, '');
}

function rewriteWebSocketHost(webSocketUrl, cdpEndpoint) {
  const normalizedEndpoint = new URL(cdpEndpoint);
  const rewritten = new URL(webSocketUrl);
  rewritten.hostname = normalizedEndpoint.hostname;
  return rewritten.toString();
}

async function createTarget(cdpEndpoint) {
  const targetUrl = `${cdpEndpoint.replace(/\/$/, '')}/json/new?${encodeURIComponent('about:blank')}`;
  let response = await fetch(targetUrl, { method: 'PUT' });
  if (response.status === 405 || response.status === 404) {
    response = await fetch(targetUrl);
  }
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`CDP target creation failed: HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  const target = await response.json();
  if (typeof target.webSocketDebuggerUrl !== 'string') {
    throw new Error('CDP target did not include webSocketDebuggerUrl');
  }
  return target;
}

async function closeTarget(cdpEndpoint, targetId) {
  if (!targetId) {
    return;
  }
  await fetch(`${cdpEndpoint.replace(/\/$/, '')}/json/close/${encodeURIComponent(targetId)}`).catch(() => {});
}

class CdpConnection {
  constructor(webSocketUrl) {
    this.nextId = 1;
    this.pending = new Map();
    this.socket = new WebSocket(webSocketUrl);
    this.socket.addEventListener('message', (event) => this.handleMessage(event));
  }

  async open(timeoutMs) {
    await withTimeout(
      new Promise((resolve, reject) => {
        this.socket.addEventListener('open', resolve, { once: true });
        this.socket.addEventListener('error', () => reject(new Error('CDP WebSocket failed to open')), {
          once: true,
        });
      }),
      timeoutMs,
      'CDP WebSocket open',
    );
  }

  handleMessage(event) {
    const message = JSON.parse(event.data);
    if (typeof message.id === 'number') {
      const pending = this.pending.get(message.id);
      if (!pending) {
        return;
      }
      this.pending.delete(message.id);
      if (message.error) {
        pending.reject(new Error(`${message.error.message ?? 'CDP error'} (${message.error.code ?? 'unknown'})`));
      } else {
        pending.resolve(message.result ?? {});
      }
      return;
    }

  }

  send(method, params = {}) {
    const id = this.nextId++;
    const promise = new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });
    this.socket.send(JSON.stringify({ id, method, params }));
    return promise;
  }

  close() {
    this.socket.close();
  }
}

function readStateExpression(options) {
  const payload = JSON.stringify({
    key: options.key,
    cookieName: options.cookieName,
  });
  return `(() => {
    const payload = ${payload};
    const cookies = Object.fromEntries(document.cookie
      .split(';')
      .map((entry) => entry.trim())
      .filter(Boolean)
      .map((entry) => {
        const separator = entry.indexOf('=');
        if (separator === -1) {
          return [entry, ''];
        }
        return [entry.slice(0, separator), decodeURIComponent(entry.slice(separator + 1))];
      }));
    return {
      href: location.href,
      localStorageValue: localStorage.getItem(payload.key),
      cookieValue: cookies[payload.cookieName] ?? '',
      cookie: document.cookie
    };
  })()`;
}

function writeStateExpression(options) {
  const payload = JSON.stringify({
    key: options.key,
    value: options.value,
    cookieName: options.cookieName,
    cookieValue: options.cookieValue,
  });
  return `(() => {
    const payload = ${payload};
    localStorage.setItem(payload.key, payload.value);
    document.cookie = payload.cookieName + '=' + encodeURIComponent(payload.cookieValue) + '; Path=/; Max-Age=86400; SameSite=Lax';
    return (${readStateExpression(options)});
  })()`;
}

async function runProbe(options) {
  const cdpEndpoint = await normalizeCdpEndpoint(options.cdpEndpoint);
  const target = await createTarget(cdpEndpoint);
  const webSocketUrl = rewriteWebSocketHost(target.webSocketDebuggerUrl, cdpEndpoint);
  const connection = new CdpConnection(webSocketUrl);
  try {
    await connection.open(options.timeoutMs);
    await connection.send('Page.enable');
    await connection.send('Runtime.enable');
    const navigation = await connection.send('Page.navigate', { url: options.originUrl });
    if (navigation.errorText) {
      throw new Error(`CDP navigation to ${options.originUrl} failed: ${navigation.errorText}`);
    }
    await waitForExpectedDocument(connection, options.originUrl, options.timeoutMs);
    const expression = options.action === 'set' ? writeStateExpression(options) : readStateExpression(options);
    const state = await evaluateValue(connection, expression);
    return {
      action: options.action,
      checkedAt: new Date().toISOString(),
      originUrl: options.originUrl,
      ...(state ?? {}),
    };
  } finally {
    connection.close();
    await closeTarget(cdpEndpoint, target.id);
  }
}

const mainUrl = process.argv[1] ? pathToFileURL(process.argv[1]).href : '';
if (import.meta.url === mainUrl) {
  const options = parseArgs(process.argv.slice(2));
  runProbe(options)
    .then((result) => {
      console.log(JSON.stringify(result, null, 2));
    })
    .catch((error) => {
      console.error(`[cdp-profile-state-probe] ${error.stack || error.message}`);
      process.exitCode = 1;
    });
}
