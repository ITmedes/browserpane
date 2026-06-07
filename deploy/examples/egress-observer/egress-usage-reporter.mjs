#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const DEFAULT_CONTAINERS = ['bpane-egress-observer', 'bpane-egress-auth-observer'];
const DEFAULT_API_URL = 'http://localhost:8080';
const DEFAULT_STATE_PATH = '.bpane-egress-usage-reporter-state.json';
const DEFAULT_SOURCE_KIND = 'proxy';
const DOCKER_TIMESTAMP_PATTERN = /^\d{4}-\d{2}-\d{2}T\S+\s+/u;
const OBSERVER_ID_PATTERN = /^[A-Za-z0-9._:-]+$/u;
const SOURCE_KINDS = new Set(['proxy', 'tls_interceptor', 'secure_web_gateway', 'custom']);

export function parseArgs(argv, env = process.env) {
  const options = {
    apiUrl: env.BPANE_API_URL ?? DEFAULT_API_URL,
    accessToken: env.BPANE_ACCESS_TOKEN ?? '',
    containers: [],
    dryRun: false,
    since: env.BPANE_EGRESS_USAGE_SINCE ?? '',
    statePath: env.BPANE_EGRESS_USAGE_STATE ?? DEFAULT_STATE_PATH,
    sourceKind: env.BPANE_EGRESS_USAGE_SOURCE_KIND ?? DEFAULT_SOURCE_KIND,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = argv[index + 1];
    if (arg === '--api-url' && next) {
      options.apiUrl = next;
      index += 1;
    } else if ((arg === '--access-token' || arg === '--token') && next) {
      options.accessToken = next;
      index += 1;
    } else if (arg === '--container' && next) {
      options.containers.push(next);
      index += 1;
    } else if (arg === '--since' && next) {
      options.since = next;
      index += 1;
    } else if (arg === '--state' && next) {
      options.statePath = next;
      index += 1;
    } else if (arg === '--source-kind' && next) {
      options.sourceKind = next;
      index += 1;
    } else if (arg === '--dry-run') {
      options.dryRun = true;
    } else if (arg === '--help') {
      options.help = true;
    } else {
      throw new Error(`Unknown or incomplete argument: ${arg}`);
    }
  }

  if (options.containers.length === 0) {
    options.containers = [...DEFAULT_CONTAINERS];
  }
  options.apiUrl = normalizeApiUrl(options.apiUrl);
  if (!SOURCE_KINDS.has(options.sourceKind)) {
    throw new Error(`Invalid source kind "${options.sourceKind}". Expected one of: ${[...SOURCE_KINDS].join(', ')}.`);
  }
  return options;
}

export function usageText() {
  return [
    'Usage: node deploy/examples/egress-observer/egress-usage-reporter.mjs [options]',
    '',
    'Options:',
    '  --api-url <url>        BrowserPane web/API origin. Default: http://localhost:8080',
    '  --access-token <token> Owner bearer token. Env: BPANE_ACCESS_TOKEN.',
    '  --container <name>     Repeatable proxy container name. Defaults to local Squid observers.',
    '  --since <duration>     Passed to docker logs --since, for example 10m or 2026-06-05T12:00:00Z.',
    '  --state <path>         Local watermark file. Default: .bpane-egress-usage-reporter-state.json',
    '  --source-kind <kind>   proxy, tls_interceptor, secure_web_gateway, or custom. Default: proxy.',
    '  --dry-run              Print sanitized reports without calling BrowserPane.',
    '  --help                 Show this help.',
    '',
    'The reporter stores only byte deltas, observer id, source kind, and timestamps in BrowserPane.',
    'Proxy URLs, response status, headers, timing, payload, and decrypted traffic stay in the proxy logs.',
  ].join('\n');
}

export function stripDockerTimestamp(line) {
  return line.replace(DOCKER_TIMESTAMP_PATTERN, '').trim();
}

export function parseSquidAccessLine(line) {
  const sanitizedLine = stripDockerTimestamp(line);
  const parts = sanitizedLine.split(/\s+/u);
  if (parts.length < 7) {
    return null;
  }

  const timestampSeconds = Number(parts[0]);
  const clientIp = parts[2];
  const responseBytes = Number(parts[4]);
  if (!Number.isFinite(timestampSeconds) || !clientIp || !Number.isSafeInteger(responseBytes) || responseBytes <= 0) {
    return null;
  }

  return {
    clientIp,
    observedAt: new Date(Math.trunc(timestampSeconds * 1000)).toISOString(),
    observedAtMs: Math.trunc(timestampSeconds * 1000),
    rxBytes: responseBytes,
    txBytes: 0,
  };
}

export function sanitizeObserverId(value) {
  const id = String(value ?? '').trim();
  if (!id || id.length > 128 || !OBSERVER_ID_PATTERN.test(id)) {
    throw new Error(`Observer id "${value}" is not compatible with BrowserPane egress usage validation.`);
  }
  return id;
}

export function runtimeIpMapFromDockerInspect(inspectRows) {
  const map = new Map();
  for (const row of inspectRows) {
    const labels = row?.Config?.Labels ?? {};
    const sessionId = labels['browserpane.session_id'];
    if (!sessionId) {
      continue;
    }
    const egressProfileId = labels['browserpane.egress_profile_id'] || null;
    const networks = row?.NetworkSettings?.Networks ?? {};
    for (const network of Object.values(networks)) {
      if (network?.IPAddress) {
        map.set(network.IPAddress, {
          sessionId,
          egressProfileId,
          containerName: String(row.Name ?? '').replace(/^\//u, ''),
        });
      }
    }
  }
  return map;
}

export function collectUsageReports({ linesByObserver, runtimeIpMap, previousState = {}, sourceKind = DEFAULT_SOURCE_KIND }) {
  const nextState = { containers: { ...(previousState.containers ?? {}) } };
  const reports = new Map();

  for (const [observerId, lines] of Object.entries(linesByObserver)) {
    const observerState = nextState.containers[observerId] ?? { lastTimestampMs: 0, seenKeys: [] };
    const previousTimestampMs = Number(observerState.lastTimestampMs ?? 0);
    const previousSeen = new Set(Array.isArray(observerState.seenKeys) ? observerState.seenKeys : []);
    let maxTimestampMs = previousTimestampMs;
    const keysAtMaxTimestamp = new Set(maxTimestampMs === previousTimestampMs ? previousSeen : []);

    for (const line of lines) {
      const parsed = parseSquidAccessLine(line);
      if (!parsed) {
        continue;
      }
      const lineKey = `${parsed.observedAtMs}:${parsed.clientIp}:${parsed.rxBytes}:${parsed.txBytes}`;
      if (parsed.observedAtMs < previousTimestampMs) {
        continue;
      }
      if (parsed.observedAtMs === previousTimestampMs && previousSeen.has(lineKey)) {
        continue;
      }
      if (parsed.observedAtMs > maxTimestampMs) {
        maxTimestampMs = parsed.observedAtMs;
        keysAtMaxTimestamp.clear();
      }
      if (parsed.observedAtMs === maxTimestampMs) {
        keysAtMaxTimestamp.add(lineKey);
      }

      const session = runtimeIpMap.get(parsed.clientIp);
      if (!session) {
        continue;
      }

      const reportKey = `${session.sessionId}:${observerId}:${sourceKind}`;
      const report = reports.get(reportKey) ?? {
        session_id: session.sessionId,
        egress_profile_id: session.egressProfileId,
        observer_id: observerId,
        source_kind: sourceKind,
        rx_bytes_delta: 0,
        tx_bytes_delta: 0,
        observed_at: parsed.observedAt,
      };
      report.rx_bytes_delta += parsed.rxBytes;
      report.tx_bytes_delta += parsed.txBytes;
      if (parsed.observedAtMs > Date.parse(report.observed_at)) {
        report.observed_at = parsed.observedAt;
      }
      reports.set(reportKey, report);
    }

    nextState.containers[observerId] = {
      lastTimestampMs: maxTimestampMs,
      seenKeys: [...keysAtMaxTimestamp].sort(),
    };
  }

  return {
    reports: [...reports.values()].filter((report) => report.rx_bytes_delta > 0 || report.tx_bytes_delta > 0),
    nextState,
  };
}

export async function runReporter(options, deps = {}) {
  const docker = deps.docker ?? defaultDocker;
  const readState = deps.readState ?? loadState;
  const writeState = deps.writeState ?? saveState;
  const postReport = deps.postReport ?? postUsageReport;
  const log = deps.log ?? console.log;
  const runtimeIpMap = runtimeIpMapFromDockerInspect(docker.inspectBrowserPaneRuntimes());
  if (runtimeIpMap.size === 0) {
    log('No BrowserPane runtime containers found. Nothing to report.');
    return { reports: [], nextState: await readState(options.statePath) };
  }

  const linesByObserver = {};
  for (const container of options.containers) {
    const observerId = sanitizeObserverId(container);
    linesByObserver[observerId] = docker.logs(container, options.since);
  }

  const previousState = await readState(options.statePath);
  const { reports, nextState } = collectUsageReports({
    linesByObserver,
    runtimeIpMap,
    previousState,
    sourceKind: options.sourceKind,
  });

  for (const report of reports) {
    if (options.dryRun) {
      log(JSON.stringify(report));
    } else {
      if (!options.accessToken) {
        throw new Error('Missing bearer token. Pass --access-token or set BPANE_ACCESS_TOKEN.');
      }
      await postReport(options, report);
      log(`reported session=${report.session_id} observer=${report.observer_id} rx=${report.rx_bytes_delta} tx=${report.tx_bytes_delta}`);
    }
  }

  if (reports.length === 0) {
    log('No new correlated egress usage lines found.');
  }
  if (!options.dryRun) {
    await writeState(options.statePath, nextState);
  }
  return { reports, nextState };
}

export async function loadState(statePath) {
  try {
    return JSON.parse(await fs.readFile(statePath, 'utf8'));
  } catch (error) {
    if (error?.code === 'ENOENT') {
      return { containers: {} };
    }
    throw error;
  }
}

export async function saveState(statePath, state) {
  await fs.mkdir(path.dirname(path.resolve(statePath)), { recursive: true });
  await fs.writeFile(statePath, `${JSON.stringify(state, null, 2)}\n`);
}

async function postUsageReport(options, report) {
  const body = {
    rx_bytes_delta: report.rx_bytes_delta,
    tx_bytes_delta: report.tx_bytes_delta,
    source_kind: report.source_kind,
    observer_id: report.observer_id,
    observed_at: report.observed_at,
  };
  const response = await fetch(`${options.apiUrl}/api/v1/sessions/${encodeURIComponent(report.session_id)}/egress-usage`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${options.accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`egress usage report failed for session ${report.session_id}: HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

const defaultDocker = {
  inspectBrowserPaneRuntimes() {
    const ids = execFileSync('docker', ['ps', '-q', '--filter', 'label=browserpane.session_id'], { encoding: 'utf8' })
      .split(/\s+/u)
      .filter(Boolean);
    if (ids.length === 0) {
      return [];
    }
    return JSON.parse(execFileSync('docker', ['inspect', ...ids], { encoding: 'utf8' }));
  },
  logs(container, since) {
    const args = ['logs', '--timestamps'];
    if (since) {
      args.push('--since', since);
    }
    args.push(container);
    return execFileSync('docker', args, { encoding: 'utf8' }).split(/\r?\n/u);
  },
};

function normalizeApiUrl(value) {
  const url = new URL(value);
  url.pathname = url.pathname.replace(/\/+$/u, '');
  url.search = '';
  url.hash = '';
  return url.toString().replace(/\/$/u, '');
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    console.log(usageText());
    return;
  }
  await runReporter(options);
}

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1]);
if (isMain) {
  main().catch((error) => {
    console.error(`[egress-usage-reporter] ${error.stack || error.message}`);
    process.exitCode = 1;
  });
}
