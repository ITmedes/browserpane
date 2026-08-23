import fs from 'node:fs';

import { ComposeArtifactCollector } from './compose-artifact-collector.mjs';
import { ComposeEvidenceAggregator } from './compose-evidence-aggregator.mjs';
import { ComposeEvidenceValidator } from './compose-evidence-validator.mjs';
import { ComposeIdentityCollector } from './compose-identity-collector.mjs';
import { ComposeJunitWriter } from './compose-junit-writer.mjs';

export class ComposeEvidenceFinalizer {
  #store;
  #rootDirectory;
  #aggregator;
  #identityCollector;
  #artifactCollector;
  #junitWriter;

  constructor(store, rootDirectory, dependencies = {}) {
    const validator = dependencies.validator ?? new ComposeEvidenceValidator();
    this.#store = store;
    this.#rootDirectory = rootDirectory;
    this.#aggregator = dependencies.aggregator ?? new ComposeEvidenceAggregator(validator);
    this.#identityCollector = dependencies.identityCollector
      ?? new ComposeIdentityCollector(rootDirectory);
    this.#artifactCollector = dependencies.artifactCollector
      ?? new ComposeArtifactCollector(rootDirectory);
    this.#junitWriter = dependencies.junitWriter ?? new ComposeJunitWriter();
  }

  finalize(lane, environment = process.env) {
    const plan = this.#store.loadPlan(lane);
    const stages = this.#store.loadStages(lane);
    const runNamespace = this.#store.loadHarness(lane)?.run_namespace ?? null;
    const identity = this.#identityCollector.collect(lane, runNamespace);
    const artifacts = this.#artifactCollector.collect(this.#store.laneDirectory(lane));
    const summary = this.#aggregator.aggregate(plan, stages, identity, artifacts, environment);
    const junit = this.#junitWriter.write(summary);
    this.#store.writeSummary(lane, summary, junit);
    this.#writeWorkflowSummary(summary, environment.GITHUB_STEP_SUMMARY);
    return summary.outcome === 'success' ? 0 : 1;
  }

  #writeWorkflowSummary(summary, summaryPath) {
    if (!summaryPath) return;
    const primary = summary.primary_failure
      ? `${summary.primary_failure.stage_id} (${summary.primary_failure.class})`
      : 'none';
    const lines = [
      `### Compose evidence: ${summary.lane}`,
      '',
      `- Outcome: ${summary.outcome}`,
      `- Primary failure: ${primary}`,
      `- Cleanup: ${summary.cleanup.outcome}`,
      `- Attempt: ${summary.run.attempt}`,
      `- Tested tree: \`${summary.identity.tested_tree}\``,
      '',
      '| Stage | Outcome | Class | Duration (ms) |',
      '| --- | --- | --- | ---: |',
      ...summary.stages.map((stage) =>
        `| ${stage.id} | ${stage.outcome} | ${stage.failure_class ?? '—'} | ${stage.duration_ms ?? '—'} |`),
      '',
    ];
    fs.appendFileSync(summaryPath, `${lines.join('\n')}\n`, { encoding: 'utf8' });
  }
}
