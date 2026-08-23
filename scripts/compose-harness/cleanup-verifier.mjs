import path from 'node:path';
import { execFileSync } from 'node:child_process';

import { ComposeHarnessError, ReadinessWaiter } from './readiness-waiter.mjs';

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u;
const DYNAMIC_CONTAINER_PATTERN = /^bpane-(?:runtime|workflow)-[a-z0-9-]+$/u;
const DYNAMIC_VOLUME_PATTERN = /^deploy_bpane-session-data-(?:browser-context-)?[a-z0-9]+$/u;
const DEFAULT_CLEANUP_TIMEOUT_MS = 60_000;
const DEFAULT_CLEAN_PASSES = 8;

export class ComposeCleanupVerifier {
  #inspector;
  #clock;
  #wait;
  #record;

  constructor({ inspector, clock = Date.now, wait, record = () => {} }) {
    if (!inspector || typeof inspector.snapshot !== 'function') {
      throw new TypeError('ComposeCleanupVerifier requires a resource inspector.');
    }
    this.#inspector = inspector;
    this.#clock = clock;
    this.#wait = wait;
    this.#record = record;
  }

  async baseline(records = []) {
    return await this.#inspector.snapshot(records);
  }

  async verify({
    namespace,
    records,
    baseline,
    timeoutMs = DEFAULT_CLEANUP_TIMEOUT_MS,
    cleanPasses = DEFAULT_CLEAN_PASSES,
    signal,
  }) {
    if (!Number.isInteger(cleanPasses) || cleanPasses <= 0) {
      throw new TypeError('Compose cleanup cleanPasses must be a positive integer.');
    }
    let consecutiveCleanPasses = 0;
    const waiter = new ReadinessWaiter({
      observe: async () => summarizeLeaks(await this.#inspector.snapshot(records), baseline),
      clock: this.#clock,
      wait: this.#wait,
    });
    try {
      const result = await waiter.waitFor('artifact', {
        timeoutMs,
        intervalMs: 250,
        signal,
        isReady: (_boundary, state) => {
          consecutiveCleanPasses = state?.available === true
            ? consecutiveCleanPasses + 1
            : 0;
          return consecutiveCleanPasses >= cleanPasses;
        },
      });
      const event = {
        type: 'cleanup',
        namespace,
        outcome: 'success',
        attempts: result.attempts,
        elapsed_ms: result.elapsed_ms,
        inventory: result.last_state,
      };
      this.#record(event);
      return event;
    } catch (error) {
      const code = error instanceof ComposeHarnessError ? error.code : 'dependency_loss';
      const event = {
        type: 'cleanup',
        namespace,
        outcome: 'failure',
        code,
        attempts: error.details?.attempts ?? 1,
        elapsed_ms: error.details?.elapsed_ms ?? 0,
        inventory: error.details?.last_state ?? { status: 'dependency_unavailable' },
      };
      this.#record(event);
      throw new ComposeHarnessError('cleanup_failure', 'Compose stage cleanup invariants failed.', event, {
        cause: error,
      });
    }
  }
}

export class ComposeResourceInspector {
  #rootDirectory;
  #execute;

  constructor(rootDirectory, execute = executeFile) {
    this.#rootDirectory = path.resolve(rootDirectory);
    this.#execute = execute;
  }

  async snapshot(records) {
    const sessionIds = records.filter((record) => record.kind === 'session').map((record) => record.id);
    const persistentVolumeIds = records
      .filter((record) => ['session', 'browser_context'].includes(record.kind))
      .map((record) => compactIdentifier(record.id));
    const temporaryVolumeIds = records
      .filter((record) => !['session', 'browser_context'].includes(record.kind))
      .map((record) => compactIdentifier(record.id));
    const identifiers = records.map((record) => compactIdentifier(record.id));
    const allContainers = this.#lines(this.#execute('docker', [
      'ps', '--all', '--format', '{{.Names}}',
    ], this.#rootDirectory, 'containers'));
    const allVolumes = this.#lines(this.#execute('docker', [
      'volume', 'ls', '--format', '{{.Name}}',
    ], this.#rootDirectory, 'volumes'));
    const activeSessions = sessionIds.length === 0 ? [] : this.#activeSessions(sessionIds);
    const retainedVolumeNames = allVolumes
      .filter((name) => persistentVolumeIds.some((id) => name.includes(id)));
    return {
      active_sessions: activeSessions.length,
      owned_containers: allContainers.filter((name) => identifiers.some((id) => name.includes(id))).length,
      owned_volumes: allVolumes
        .filter((name) => temporaryVolumeIds.some((id) => name.includes(id))).length,
      retained_volumes: retainedVolumeNames.length,
      retained_volume_names: retainedVolumeNames,
      dynamic_containers: allContainers.filter((name) => DYNAMIC_CONTAINER_PATTERN.test(name)),
      dynamic_volumes: allVolumes.filter((name) => DYNAMIC_VOLUME_PATTERN.test(name)),
    };
  }

  #activeSessions(sessionIds) {
    const validated = [...new Set(sessionIds)].filter((id) => UUID_PATTERN.test(id));
    if (validated.length !== new Set(sessionIds).size) {
      throw new ComposeHarnessError('stale_resource', 'Session registry contained an invalid id.');
    }
    const quoted = validated.map((id) => `'${id}'`).join(',');
    const query = `SELECT id::text || ':' || state::text FROM control_sessions WHERE id IN (${quoted})`;
    const output = this.#execute('docker', [
      'compose', '-f', path.join(this.#rootDirectory, 'deploy/compose.yml'),
      'exec', '-T', 'postgres', 'psql', '--tuples-only', '--no-align',
      '--username', 'browserpane', '--dbname', 'browserpane', '--command', query,
    ], this.#rootDirectory, 'session_store');
    return this.#lines(output).filter((line) => !line.endsWith(':stopped'));
  }

  #lines(output) {
    return String(output).split('\n').map((line) => line.trim()).filter(Boolean);
  }
}

function summarizeLeaks(snapshot, baseline) {
  const baselineContainers = new Set(baseline.dynamic_containers ?? []);
  const baselineVolumes = new Set(baseline.dynamic_volumes ?? []);
  const retainedVolumes = new Set(snapshot.retained_volume_names ?? []);
  const newContainers = (snapshot.dynamic_containers ?? [])
    .filter((name) => !baselineContainers.has(name)).length;
  const newVolumes = (snapshot.dynamic_volumes ?? [])
    .filter((name) => !baselineVolumes.has(name) && !retainedVolumes.has(name)).length;
  const summary = {
    status: 'cleanup_pending',
    active_sessions: snapshot.active_sessions ?? 0,
    owned_containers: snapshot.owned_containers ?? 0,
    owned_volumes: snapshot.owned_volumes ?? 0,
    retained_volumes: snapshot.retained_volumes ?? 0,
    unexpected_containers: newContainers,
    unexpected_volumes: newVolumes,
  };
  summary.available = Object.entries(summary)
    .filter(([key]) => !['status', 'available', 'retained_volumes'].includes(key))
    .every(([, value]) => value === 0);
  if (summary.available) summary.status = 'clean';
  return summary;
}

function compactIdentifier(id) {
  return String(id).toLowerCase().replaceAll('-', '');
}

function executeFile(command, args, cwd, boundary) {
  try {
    return execFileSync(command, args, {
      cwd,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      timeout: 10_000,
    });
  } catch (error) {
    throw new ComposeHarnessError('dependency_loss', `Compose ${boundary} inventory is unavailable.`, {}, {
      cause: error,
    });
  }
}
