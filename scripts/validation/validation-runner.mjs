export class ValidationRunner {
  #executor;
  #logger;
  #cancelledSignal = null;

  constructor(executor, logger = console) {
    this.#executor = executor;
    this.#logger = logger;
  }

  async run(stages, options = {}) {
    this.#logger.log(`Validation stages: ${stages.map((stage) => stage.id).join(', ')}`);
    if (options.dryRun) {
      for (const stage of stages) this.#printStage('PLAN', stage);
      return 0;
    }

    for (const stage of stages) {
      if (this.#cancelledSignal) return this.#signalExitCode(this.#cancelledSignal);
      this.#printStage('START', stage);
      const startedAt = Date.now();
      const result = await this.#executor.execute(stage);
      const duration = ((Date.now() - startedAt) / 1000).toFixed(1);
      if (result.exitCode !== 0) {
        const reason = result.timedOut
          ? `timeout after ${stage.timeoutMs / 1000}s`
          : `exit ${result.exitCode}`;
        this.#logger.error(`[validate] FAIL ${stage.id} (${reason}, ${duration}s)`);
        this.#logger.error(`[validate] rerun: ${this.#rerunCommand(stage.id)}`);
        return result.exitCode;
      }
      this.#logger.log(`[validate] PASS ${stage.id} (${duration}s)`);
    }

    this.#logger.log(`[validate] PASS all ${stages.length} stages`);
    return 0;
  }

  cancel(signal) {
    this.#cancelledSignal = signal;
    this.#executor.cancel(signal);
  }

  #printStage(prefix, stage) {
    this.#logger.log(`[validate] ${prefix} ${stage.id}: ${stage.description}`);
    this.#logger.log(`[validate] command: cd ${stage.cwd} && ${stage.commandLine()}`);
  }

  #rerunCommand(stageId) {
    return `node scripts/validate.mjs --stage ${stageId}`;
  }

  #signalExitCode(signal) {
    if (signal === 'SIGINT') return 130;
    if (signal === 'SIGTERM') return 143;
    return 1;
  }
}
