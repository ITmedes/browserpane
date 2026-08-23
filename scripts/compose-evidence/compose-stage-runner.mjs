import { spawn } from 'node:child_process';

import { ComposeFailureClassifier } from './compose-failure-classifier.mjs';
import { ComposeNamespaceFactory } from '../compose-harness/compose-namespace.mjs';
import { ComposeStageHarness } from '../compose-harness/compose-stage-harness.mjs';

const SIGNAL_EXIT_CODES = { SIGINT: 130, SIGTERM: 143 };

export class ComposeStageRunner {
  #store;
  #classifier;
  #clock;
  #workingDirectory;

  constructor(store, workingDirectory, clock = () => Date.now()) {
    this.#store = store;
    this.#workingDirectory = workingDirectory;
    this.#clock = clock;
    this.#classifier = new ComposeFailureClassifier();
  }

  async run(lane, stageId, command) {
    if (!Array.isArray(command) || command.length === 0) {
      throw new Error('Compose evidence stage command is required');
    }
    const plan = this.#store.loadPlan(lane);
    const stage = plan.stages.find((candidate) => candidate.id === stageId);
    if (!stage) throw new Error(`stage ${stageId} is not in lane ${lane}`);

    const started = this.#clock();
    const startedAt = new Date(started).toISOString();
    const harnessConfig = this.#store.loadHarness(lane);
    const runNamespace = harnessConfig?.run_namespace
      ?? new ComposeNamespaceFactory().run(lane);
    const harnessResult = await new ComposeStageHarness(
      this.#workingDirectory,
      this.#store.stateDirectory(lane),
    ).run({
      lane,
      stage,
      command,
      execute: (stageCommand, environment) => this.#execute(stageCommand, environment),
      runNamespace,
    });
    const execution = harnessResult.execution;
    const classifiedInput = harnessResult.cleanupError && execution.exitCode === 0
      ? { ...execution, exitCode: 1, defaultClass: 'harness' }
      : { ...execution, defaultClass: stage.failure_class };
    const classified = this.#classifier.classify({
      ...classifiedInput,
    });
    const result = {
      ...stage,
      outcome: classified.outcome,
      failure_class: classified.failureClass,
      started_at: startedAt,
      duration_ms: Math.min(Math.max(0, this.#clock() - started), 21_600_000),
      exit_code: classifiedInput.exitCode,
      signal: execution.requestedSignal ?? execution.signal,
      harness: harnessResult.harness,
    };
    this.#store.writeStage(lane, result);
    if (execution.exitCode === 0 && harnessResult.cleanupError) return 1;
    return execution.exitCode ?? SIGNAL_EXIT_CODES[result.signal] ?? 1;
  }

  #execute(command, environment) {
    return new Promise((resolve) => {
      let requestedSignal = null;
      let spawnError = false;
      const child = spawn(command[0], command.slice(1), {
        cwd: this.#workingDirectory,
        env: environment,
        stdio: 'inherit',
      });
      const forward = (signal) => {
        requestedSignal = signal;
        if (!child.killed) child.kill(signal);
      };
      const onSigint = () => forward('SIGINT');
      const onSigterm = () => forward('SIGTERM');
      process.on('SIGINT', onSigint);
      process.on('SIGTERM', onSigterm);
      child.once('error', () => {
        spawnError = true;
      });
      child.once('close', (exitCode, signal) => {
        process.removeListener('SIGINT', onSigint);
        process.removeListener('SIGTERM', onSigterm);
        resolve({ exitCode, signal, spawnError, requestedSignal });
      });
    });
  }
}
