import process from 'node:process';
import { lookup } from 'node:dns/promises';
import net from 'node:net';

const DEFAULTS = {
  cdpEndpoint: '',
  probeUrl: 'file:///etc/passwd',
  timeoutMs: 10000,
};

function parseArgs(argv) {
  const options = { ...DEFAULTS };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const next = argv[i + 1];
    if (arg === '--cdp-endpoint' && next) {
      options.cdpEndpoint = next;
      i++;
    } else if (arg === '--probe-url' && next) {
      options.probeUrl = next;
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
  return options;
}

function printHelp() {
  console.log(`
Usage: node scripts/cdp-local-file-policy-probe.mjs --cdp-endpoint <url> [options]

Options:
  --cdp-endpoint <url>  Chromium DevTools endpoint, for example http://host:9223
  --probe-url <url>     Local file URL to probe (default: ${DEFAULTS.probeUrl})
  --timeout-ms <ms>     Probe timeout (default: ${DEFAULTS.timeoutMs})
  --help               Show this help
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

async function createTarget(cdpEndpoint) {
  const encodedUrl = encodeURIComponent('about:blank');
  const targetUrl = `${cdpEndpoint.replace(/\/$/, '')}/json/new?${encodedUrl}`;
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
    this.eventWaiters = new Map();
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

    const waiters = this.eventWaiters.get(message.method);
    if (!waiters?.length) {
      return;
    }
    const waiter = waiters.shift();
    waiter.resolve(message.params ?? {});
  }

  send(method, params = {}) {
    const id = this.nextId++;
    const payload = JSON.stringify({ id, method, params });
    const promise = new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });
    this.socket.send(payload);
    return promise;
  }

  waitForEvent(method, timeoutMs) {
    const promise = new Promise((resolve, reject) => {
      const waiters = this.eventWaiters.get(method) ?? [];
      waiters.push({ resolve, reject });
      this.eventWaiters.set(method, waiters);
    });
    return withTimeout(promise, timeoutMs, method);
  }

  close() {
    this.socket.close();
  }
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
    const loadEvent = connection.waitForEvent('Page.loadEventFired', options.timeoutMs).catch(() => null);
    await connection.send('Page.navigate', { url: options.probeUrl });
    await loadEvent;
    const evaluation = await connection.send('Runtime.evaluate', {
      expression: `(() => ({
        href: location.href,
        title: document.title,
        bodyText: document.body ? document.body.innerText : '',
        documentText: document.documentElement ? document.documentElement.innerText : ''
      }))()`,
      returnByValue: true,
    });
    const value = evaluation?.result?.value ?? {};
    const visibleText = `${value.title ?? ''}\n${value.bodyText ?? ''}\n${value.documentText ?? ''}`;
    const exposedPasswd = /(^|\n)root:[^:\n]*:0:0:/m.test(visibleText);
    return {
      blocked: !exposedPasswd,
      checkedAt: new Date().toISOString(),
      probeUrl: options.probeUrl,
      finalUrl: String(value.href ?? ''),
      title: String(value.title ?? ''),
      reason: exposedPasswd ? 'probe content exposed /etc/passwd root entry' : '',
      visibleTextSample: visibleText.slice(0, 600),
    };
  } finally {
    connection.close();
    await closeTarget(cdpEndpoint, target.id);
  }
}

const options = parseArgs(process.argv.slice(2));
runProbe(options)
  .then((result) => {
    console.log(JSON.stringify(result, null, 2));
    if (!result.blocked) {
      process.exitCode = 1;
    }
  })
  .catch((error) => {
    console.error(`[cdp-local-file-policy-probe] ${error.stack || error.message}`);
    process.exitCode = 1;
  });
