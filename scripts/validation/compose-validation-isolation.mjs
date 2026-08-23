import path from 'node:path';

import { ComposeHarnessRecorder } from '../compose-harness/harness-recorder.mjs';
import { ComposeNamespaceFactory } from '../compose-harness/compose-namespace.mjs';
import { ComposeStageHarness } from '../compose-harness/compose-stage-harness.mjs';

export class ComposeValidationIsolation {
  #rootDirectory;
  #environment;
  #runNamespace;
  #harnessFactory;

  constructor(rootDirectory, environment = process.env, dependencies = {}) {
    this.#rootDirectory = path.resolve(rootDirectory);
    this.#environment = environment;
    const namespaceFactory = dependencies.namespaceFactory ?? new ComposeNamespaceFactory();
    this.#runNamespace = environment.BPANE_CI_STAGE_NAMESPACE
      ?? namespaceFactory.run('local-compose-validation', environment);
    this.#harnessFactory = dependencies.harnessFactory ?? ((stateDirectory) =>
      new ComposeStageHarness(this.#rootDirectory, stateDirectory));
  }

  async execute(stage, execute) {
    const stateDirectory = path.join(
      this.#rootDirectory,
      'test-results/compose-harness',
      this.#runNamespace,
    );
    const result = await this.#harnessFactory(stateDirectory).run({
      lane: this.#environment.BPANE_CI_EVIDENCE_LANE ?? 'local-compose-validation',
      stage,
      command: [stage.command, ...stage.args],
      execute: (_command, environment) => execute(stage, environment),
      runNamespace: this.#runNamespace,
      environment: this.#environment,
    });
    this.#forwardEvidence(result.harness);
    return result;
  }

  #forwardEvidence(harness) {
    const recorder = new ComposeHarnessRecorder(this.#environment.BPANE_CI_HARNESS_EVENTS);
    for (const boundary of harness.readiness.boundaries) {
      recorder.record({
        type: 'readiness',
        namespace: harness.namespace,
        boundary,
        outcome: harness.readiness.failures === 0 ? 'ready' : 'failure',
        attempts: harness.readiness.attempts,
        elapsed_ms: harness.readiness.elapsed_ms,
        last_state: { status: harness.readiness.last_state },
      });
    }
    recorder.record({
      type: 'cleanup',
      namespace: harness.namespace,
      outcome: harness.cleanup.outcome,
      attempts: harness.cleanup.attempts,
      elapsed_ms: harness.cleanup.elapsed_ms,
      inventory: { status: harness.cleanup.last_state },
    });
  }
}
