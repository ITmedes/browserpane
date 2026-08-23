import { spawn } from 'node:child_process';

import { ComposeFailureClassifier } from './compose-failure-classifier.mjs';

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
    const execution = await this.#execute(command);
    const classified = this.#classifier.classify({
      ...execution,
      defaultClass: stage.failure_class,
    });
    const result = {
      ...stage,
      outcome: classified.outcome,
      failure_class: classified.failureClass,
      started_at: startedAt,
      duration_ms: Math.min(Math.max(0, this.#clock() - started), 21_600_000),
      exit_code: execution.exitCode,
      signal: execution.requestedSignal ?? execution.signal,
    };
    this.#store.writeStage(lane, result);
    return execution.exitCode ?? SIGNAL_EXIT_CODES[result.signal] ?? 1;
  }

  #execute(command) {
    return new Promise((resolve) => {
      let requestedSignal = null;
      let spawnError = false;
      const child = spawn(command[0], command.slice(1), {
        cwd: this.#workingDirectory,
        env: process.env,
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
