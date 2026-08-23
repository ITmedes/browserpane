import { ComposeEvidenceAggregator } from '../compose-evidence/compose-evidence-aggregator.mjs';
import { ComposeEvidenceValidator } from '../compose-evidence/compose-evidence-validator.mjs';
import { ComposeLanePlanCatalog } from '../compose-evidence/compose-lane-plans.mjs';

export class ComposeEvidenceFixture {
  #plan;

  constructor(lane = 'gateway-default') {
    this.#plan = new ComposeLanePlanCatalog().plan(lane);
  }

  plan() {
    return structuredClone(this.#plan);
  }

  summary(options = {}) {
    const results = this.results(options);
    const aggregator = new ComposeEvidenceAggregator(
      new ComposeEvidenceValidator(),
      () => new Date('2026-08-23T12:00:00.000Z')
    );
    return aggregator.aggregate(
      this.plan(),
      results,
      options.identityResult ?? this.identityResult(),
      options.artifactResult ?? { artifacts: [], errors: [] },
      { GITHUB_RUN_ID: '12345', GITHUB_RUN_ATTEMPT: '1', GITHUB_EVENT_NAME: 'schedule' },
    );
  }

  results(options = {}) {
    const failureId = options.failureId ?? null;
    const cleanupOutcome = options.cleanupOutcome ?? 'success';
    const failureStage = this.#plan.stages.find((stage) => stage.id === failureId);
    const results = [];
    for (const stage of this.#plan.stages) {
      if (stage.role === 'primary') {
        if (failureStage && stage.sequence > failureStage.sequence) continue;
        results.push(this.#result(stage, stage.id === failureId ? 'failure' : 'success'));
      } else if (stage.role === 'diagnostics') {
        if (failureId && options.omitDiagnostics !== true) results.push(this.#result(stage, 'success'));
      } else if (options.omitCleanup !== true) {
        results.push(this.#result(stage, cleanupOutcome));
      }
    }
    return results;
  }

  identityResult() {
    return {
      identity: {
        tested_commit: '1'.repeat(40),
        tested_tree: '2'.repeat(40),
        workflow_sha256: '3'.repeat(64),
        test_plan_sha256: '4'.repeat(64),
        image_digests: [`sha256:${'5'.repeat(64)}`],
      },
      errors: [],
    };
  }

  #result(stage, outcome) {
    return {
      ...stage,
      outcome,
      failure_class: outcome === 'success' ? null : stage.failure_class,
      started_at: '2026-08-23T11:59:00.000Z',
      duration_ms: 250,
      exit_code: outcome === 'success' ? 0 : 1,
      signal: null,
    };
  }
}
