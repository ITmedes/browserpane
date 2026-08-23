import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { ComposeEvidenceFinalizer } from '../compose-evidence/compose-evidence-finalizer.mjs';
import { ComposeEvidenceStore } from '../compose-evidence/compose-evidence-store.mjs';
import { ComposeStageRunner } from '../compose-evidence/compose-stage-runner.mjs';
import { ComposeEvidenceFixture } from './compose-evidence-fixture.mjs';

test('stage runner records elapsed time and exit semantics without command output', async (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-compose-runner-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const store = new ComposeEvidenceStore(directory);
  const fixture = new ComposeEvidenceFixture();
  store.initialize(fixture.plan());
  const times = [1_000, 1_250];
  const runner = new ComposeStageRunner(store, directory, () => times.shift());

  const exitCode = await runner.run('gateway-default', 'registry-auth', [
    process.execPath, '-e', 'process.exit(0)',
  ]);
  const [result] = store.loadStages('gateway-default');

  assert.equal(exitCode, 0);
  assert.equal(result.outcome, 'success');
  assert.equal(result.failure_class, null);
  assert.equal(result.duration_ms, 250);
  assert.equal('command' in result, false);
});

test('finalizer writes JSON, JUnit, and a bounded workflow summary', (t) => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-compose-finalizer-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const store = new ComposeEvidenceStore(directory);
  const fixture = new ComposeEvidenceFixture();
  const plan = fixture.plan();
  store.initialize(plan);
  for (const result of fixture.results()) store.writeStage(plan.lane, result);
  const workflowSummary = path.join(directory, 'workflow-summary.md');
  const finalizer = new ComposeEvidenceFinalizer(store, directory, {
    identityCollector: { collect: () => fixture.identityResult() },
    artifactCollector: { collect: () => ({ artifacts: [], errors: [] }) },
  });

  const exitCode = finalizer.finalize(plan.lane, {
    GITHUB_RUN_ID: '123',
    GITHUB_RUN_ATTEMPT: '1',
    GITHUB_EVENT_NAME: 'schedule',
    GITHUB_STEP_SUMMARY: workflowSummary,
  });
  const laneDirectory = store.laneDirectory(plan.lane);
  const json = JSON.parse(fs.readFileSync(
    path.join(laneDirectory, 'compose-stage-evidence-v1.json'), 'utf8'
  ));
  const junit = fs.readFileSync(path.join(laneDirectory, 'compose-stage-evidence-v1.xml'), 'utf8');

  assert.equal(exitCode, 0);
  assert.equal(json.outcome, 'success');
  assert.match(junit, /<testsuites/);
  assert.match(fs.readFileSync(workflowSummary, 'utf8'), /Cleanup: success/);
});
