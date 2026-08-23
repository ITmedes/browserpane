import assert from 'node:assert/strict';
import test from 'node:test';

import { ComposeEvidenceAggregator } from '../compose-evidence/compose-evidence-aggregator.mjs';
import { ComposeEvidenceValidator } from '../compose-evidence/compose-evidence-validator.mjs';
import { ComposeJunitWriter } from '../compose-evidence/compose-junit-writer.mjs';
import { ComposeEvidenceFixture } from './compose-evidence-fixture.mjs';

test('successful evidence serializes bounded JSON and JUnit with skipped diagnostics', () => {
  const summary = new ComposeEvidenceFixture().summary();
  const diagnostics = summary.stages.find((stage) => stage.id === 'diagnostics');
  const junit = new ComposeJunitWriter().write(summary);

  assert.equal(summary.outcome, 'success');
  assert.equal(summary.primary_failure, null);
  assert.equal(summary.cleanup.outcome, 'success');
  assert.equal(diagnostics.outcome, 'skipped');
  assert.deepEqual(summary.evidence_errors, []);
  assert.deepEqual(new ComposeEvidenceValidator().validateSummary(summary), []);
  assert.match(junit, /testsuites name="BrowserPane Compose evidence"/);
  assert.match(junit, /name="diagnostics"[^<]*><skipped \/>/);
});

test('product failure remains primary when cleanup also fails', () => {
  const summary = new ComposeEvidenceFixture().summary({
    failureId: 'gateway-suite',
    cleanupOutcome: 'failure',
  });

  assert.equal(summary.outcome, 'failure');
  assert.deepEqual(summary.primary_failure, {
    stage_id: 'gateway-suite',
    class: 'product',
    outcome: 'failure',
    exit_code: 1,
    signal: null,
  });
  assert.equal(summary.cleanup.outcome, 'failure');
  assert.match(new ComposeJunitWriter().write(summary), /name="cleanup"/);
});

test('missing failure diagnostics fail closed without replacing the primary failure', () => {
  const summary = new ComposeEvidenceFixture().summary({
    failureId: 'gateway-suite',
    omitDiagnostics: true,
  });

  assert.equal(summary.primary_failure.stage_id, 'gateway-suite');
  assert.equal(summary.primary_failure.class, 'product');
  assert.equal(summary.stages.find((stage) => stage.id === 'diagnostics').outcome, 'unknown');
  assert.ok(summary.evidence_errors.includes('required-stage-missing-diagnostics'));
});

test('missing required stage becomes a terminal unknown failure', () => {
  const fixture = new ComposeEvidenceFixture();
  const results = fixture.results().filter((stage) => stage.id !== 'native-prerequisites');
  const aggregator = new ComposeEvidenceAggregator(new ComposeEvidenceValidator(),
    () => new Date('2026-08-23T12:00:00.000Z'));
  const summary = aggregator.aggregate(fixture.plan(), results, fixture.identityResult(),
    { artifacts: [], errors: [] },
    { GITHUB_RUN_ID: '9', GITHUB_RUN_ATTEMPT: '1', GITHUB_EVENT_NAME: 'schedule' });

  assert.equal(summary.outcome, 'failure');
  assert.equal(summary.primary_failure.stage_id, 'native-prerequisites');
  assert.equal(summary.primary_failure.class, 'unknown');
});

test('retry attempt is explicit and never represented as first pass', () => {
  const fixture = new ComposeEvidenceFixture();
  const aggregator = new ComposeEvidenceAggregator(new ComposeEvidenceValidator(),
    () => new Date('2026-08-23T12:00:00.000Z'));
  const summary = aggregator.aggregate(fixture.plan(), fixture.results(), fixture.identityResult(),
    { artifacts: [], errors: [] },
    { GITHUB_RUN_ID: '9', GITHUB_RUN_ATTEMPT: '2', GITHUB_EVENT_NAME: 'workflow_dispatch' });

  assert.equal(summary.run.attempt, 2);
  assert.equal(summary.run.first_pass, false);
});

test('cancelled primary stage remains terminal and distinct from cleanup', () => {
  const fixture = new ComposeEvidenceFixture();
  const results = fixture.results({ failureId: 'gateway-suite' });
  const cancelled = results.find((stage) => stage.id === 'gateway-suite');
  cancelled.outcome = 'cancelled';
  cancelled.failure_class = 'infrastructure';
  cancelled.exit_code = null;
  cancelled.signal = 'SIGTERM';
  const summary = new ComposeEvidenceAggregator(new ComposeEvidenceValidator(),
    () => new Date('2026-08-23T12:00:00.000Z')).aggregate(
    fixture.plan(), results, fixture.identityResult(), { artifacts: [], errors: [] },
    { GITHUB_RUN_ID: '9', GITHUB_RUN_ATTEMPT: '1', GITHUB_EVENT_NAME: 'push' }
  );

  assert.equal(summary.outcome, 'cancelled');
  assert.equal(summary.primary_failure.class, 'infrastructure');
  assert.equal(summary.cleanup.outcome, 'success');
});
