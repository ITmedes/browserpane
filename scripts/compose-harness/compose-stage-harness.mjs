import fs from 'node:fs';
import path from 'node:path';

import { ComposeCleanupVerifier, ComposeResourceInspector } from './cleanup-verifier.mjs';
import { ComposeHarnessRecorder } from './harness-recorder.mjs';
import { ComposeNamespaceFactory } from './compose-namespace.mjs';
import { ComposeResourceRegistry } from './resource-registry.mjs';
import { ComposeHarnessError } from './readiness-waiter.mjs';

export class ComposeStageHarness {
  #rootDirectory;
  #stateDirectory;
  #namespaceFactory;
  #verifierFactory;

  constructor(rootDirectory, stateDirectory, dependencies = {}) {
    this.#rootDirectory = path.resolve(rootDirectory);
    this.#stateDirectory = path.resolve(stateDirectory);
    this.#namespaceFactory = dependencies.namespaceFactory ?? new ComposeNamespaceFactory();
    this.#verifierFactory = dependencies.verifierFactory ?? ((record) => new ComposeCleanupVerifier({
      inspector: new ComposeResourceInspector(this.#rootDirectory),
      record,
    }));
  }

  async run({ lane, stage, command, execute, runNamespace, environment = process.env }) {
    const namespace = this.#namespaceFactory.stage(runNamespace, stage.id);
    const paths = this.#paths(stage.id);
    fs.mkdirSync(path.dirname(paths.events), { recursive: true, mode: 0o700 });
    const recorder = new ComposeHarnessRecorder(paths.events);
    const registry = new ComposeResourceRegistry(paths.registry, namespace);
    const verifier = stage.isolation === 'resources'
      ? this.#verifierFactory((event) => recorder.record(event))
      : null;
    let baseline = null;
    let preparationError = null;
    if (verifier) {
      try {
        baseline = await verifier.baseline();
        const priorRecords = this.#priorResourceRecords(paths.registry);
        const priorState = await verifier.baseline(priorRecords);
        if ((priorState.active_sessions ?? 0) > 0
          || (priorState.owned_containers ?? 0) > 0
          || (priorState.owned_volumes ?? 0) > 0) {
          preparationError = new ComposeHarnessError(
            'cleanup_failure',
            'Compose stage refused leaked resources from an earlier isolated stage.',
          );
        }
      } catch (error) {
        preparationError = error;
      }
    }
    if (preparationError) {
      recorder.record({
        type: 'cleanup',
        namespace,
        outcome: 'failure',
        code: preparationError.code ?? 'dependency_loss',
        attempts: 1,
        elapsed_ms: 0,
        inventory: {
          status: baseline ? 'non_empty_baseline' : 'dependency_unavailable',
        },
      });
    }

    const childEnvironment = {
      ...environment,
      BPANE_CI_RUN_NAMESPACE: runNamespace,
      BPANE_CI_STAGE_NAMESPACE: namespace,
      BPANE_CI_EVIDENCE_LANE: lane,
      BPANE_CI_HARNESS_EVENTS: paths.events,
      BPANE_CI_RESOURCE_REGISTRY: paths.registry,
      NODE_OPTIONS: fs.existsSync(this.#preloadPath())
        ? appendNodeImport(environment.NODE_OPTIONS, this.#preloadPath())
        : environment.NODE_OPTIONS,
    };
    const execution = preparationError
      ? { exitCode: 1, signal: null, spawnError: false, requestedSignal: null }
      : await execute(command, childEnvironment);
    let cleanupError = preparationError;
    let records = [];
    if (verifier && !preparationError) {
      try {
        records = registry.load();
        await verifier.verify({
          namespace,
          records,
          baseline,
        });
      } catch (error) {
        cleanupError = error;
      }
    }
    try {
      this.#forwardResources(records, environment, paths.registry);
    } catch (error) {
      cleanupError ??= new ComposeHarnessError(
        'cleanup_failure',
        'Compose nested harness ownership propagation failed.',
        {},
        { cause: error },
      );
      recorder.record({
        type: 'cleanup',
        namespace,
        outcome: 'failure',
        code: 'cleanup_failure',
        attempts: 1,
        elapsed_ms: 0,
        inventory: { status: 'ownership_propagation_failed' },
      });
    }

    return {
      execution,
      cleanupError,
      harness: {
        ...summarizeHarness(namespace, recorder.load()),
      },
    };
  }

  #paths(stageId) {
    const directory = path.join(this.#stateDirectory, 'harness');
    return {
      events: path.join(directory, `${stageId}-events.jsonl`),
      registry: path.join(directory, `${stageId}-resources.jsonl`),
    };
  }

  #preloadPath() {
    return path.join(this.#rootDirectory, 'scripts/compose-harness/preload.mjs');
  }

  #forwardResources(records, environment, registryPath) {
    const parentPath = environment.BPANE_CI_RESOURCE_REGISTRY;
    const parentNamespace = environment.BPANE_CI_STAGE_NAMESPACE;
    if (!parentPath || !parentNamespace || path.resolve(parentPath) === path.resolve(registryPath)) {
      return;
    }
    const parent = new ComposeResourceRegistry(parentPath, parentNamespace);
    for (const record of records) parent.record(record.kind, record.id);
  }

  #priorResourceRecords(currentRegistryPath) {
    const directory = path.dirname(currentRegistryPath);
    if (!fs.existsSync(directory)) return [];
    return fs.readdirSync(directory)
      .filter((name) => name.endsWith('-resources.jsonl'))
      .map((name) => path.join(directory, name))
      .filter((filePath) => filePath !== currentRegistryPath)
      .flatMap((filePath) => {
        const firstLine = fs.readFileSync(filePath, 'utf8').split('\n').find(Boolean);
        if (!firstLine) return [];
        const namespace = JSON.parse(firstLine).namespace;
        return new ComposeResourceRegistry(filePath, namespace).load();
      });
  }
}

function summarizeHarness(namespace, events) {
  const readinessEvents = events.filter((event) => event.type === 'readiness');
  const cleanupEvents = events.filter((event) => event.type === 'cleanup');
  const cleanupFailures = cleanupEvents.filter((event) => event.outcome !== 'success');
  return {
    namespace,
    namespaces: [...new Set([namespace, ...events.map((event) => event.namespace).filter(Boolean)])]
      .slice(0, 64),
    readiness: {
      boundaries: [...new Set(readinessEvents.map((event) => event.boundary).filter(Boolean))],
      observations: readinessEvents.length,
      attempts: readinessEvents.reduce((total, event) => total + (event.attempts ?? 0), 0),
      failures: readinessEvents.filter((event) => event.outcome !== 'ready').length,
      elapsed_ms: readinessEvents.reduce((maximum, event) =>
        Math.max(maximum, event.elapsed_ms ?? 0), 0),
      last_state: readinessEvents.at(-1)?.last_state?.status ?? null,
    },
    cleanup: {
      outcome: cleanupFailures.length === 0 ? 'success' : 'failure',
      checks: cleanupEvents.length,
      failures: cleanupFailures.length,
      attempts: cleanupEvents.reduce((total, event) => total + (event.attempts ?? 0), 0),
      elapsed_ms: cleanupEvents.reduce((maximum, event) =>
        Math.max(maximum, event.elapsed_ms ?? 0), 0),
      last_state: cleanupEvents.at(-1)?.inventory?.status ?? null,
    },
  };
}

function appendNodeImport(existing, preloadPath) {
  const option = `--import=${preloadPath}`;
  return existing ? `${existing} ${option}` : option;
}
