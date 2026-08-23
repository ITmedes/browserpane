import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';

import {
  COMPATIBILITY_ADMIN_PROMOTION_SMOKES,
  UNIFIED_ADMIN_PROMOTION_SMOKES,
} from '../validation/admin-promotion-contract.mjs';
import { ComposeLanePlanCatalog } from '../compose-evidence/compose-lane-plans.mjs';
import { YamlDocumentParser } from '../validation/yaml-document-parser.mjs';

const root = path.resolve(import.meta.dirname, '../..');
const workflow = new YamlDocumentParser().parse(
  path.join(root, '.github/workflows/compose.yml')
);

function steps(job) {
  return job.steps ?? [];
}

function stepByName(job, name) {
  return steps(job).find((step) => step.name === name);
}

test('compose workflow isolates both gateway API suites', () => {
  const job = workflow.jobs['gateway-api'];

  assert.ok(job);
  assert.equal(job.strategy['fail-fast'], false);
  assert.deepEqual(job.strategy.matrix.suite, ['default', 'docker-pool']);
  assert.match(
    stepByName(job, 'Run gateway compose API suite').run,
    /--suite \$\{\{ matrix\.suite \}\}/
  );
});

test('compose workflow preserves every browser-facing smoke stage', () => {
  const job = workflow.jobs['browser-integrations'];
  const command = stepByName(job, 'Run browser-facing compose validation').run;
  const expectedStages = [
    'compose-cli',
    'compose-session-files',
    'compose-mcp',
    'compose-recording',
    'compose-workflow',
    'compose-workflow-cli',
    'compose-workflow-workspace',
    'compose-workflow-events'
  ];

  assert.equal(
    stepByName(job, 'Prepare compose runtime').run.includes(
      'scripts/run-gateway-compose-e2e.sh --suite stack'
    ),
    true
  );
  for (const stage of expectedStages) {
    assert.match(command, new RegExp(`--stage ${stage}(?:\\s|$)`));
  }
  assert.equal((command.match(/--stage compose-/g) ?? []).length, expectedStages.length);
});

test('compose workflow runs unified and compatibility promotion lanes independently', () => {
  const job = workflow.jobs['admin-promotion'];
  const fixtureStep = stepByName(job, 'Prepare egress observer fixtures');

  assert.ok(job);
  assert.equal(job.strategy['fail-fast'], false);
  assert.deepEqual(job.strategy.matrix.surface, ['unified', 'compatibility']);
  assert.match(stepByName(job, 'Run admin promotion validation').run,
    /run-admin-promotion-validation\.mjs "\$\{\{ matrix\.surface \}\}"/);
  assert.equal(fixtureStep.if, "matrix.surface == 'compatibility'");
  assert.match(fixtureStep.run, /--stage egress-fixtures --/);
  assert.match(fixtureStep.run, /scripts\/ci\/start-compose-egress-fixtures\.sh/);
  assert.ok(UNIFIED_ADMIN_PROMOTION_SMOKES.length > 0);
  assert.ok(COMPATIBILITY_ADMIN_PROMOTION_SMOKES.length > 0);
});

test('gateway validation uses preinstalled native tools without a host package manager', () => {
  const job = workflow.jobs['gateway-api'];
  const prerequisites = stepByName(job, 'Verify native build prerequisites');

  for (const tool of ['cc', 'c++', 'make', 'cmake', 'pkg-config']) {
    assert.match(prerequisites.run, new RegExp(`(?:^|\\s)${tool.replaceAll('+', '\\+')}(?:;|\\s)`));
  }
  assert.match(prerequisites.run, /command -v "\$tool"/);
  assert.doesNotMatch(
    JSON.stringify(workflow),
    /(?:apt-get|apt |dnf |yum |brew install)/,
  );
});

test('every compose lane retains cancellation-aware diagnostics and unconditional evidence', () => {
  for (const [jobId, job] of Object.entries(workflow.jobs)) {
    const collect = stepByName(job, 'Collect redacted failure diagnostics');
    const cleanup = stepByName(job, 'Clean compose resources');
    const finalize = stepByName(job, 'Finalize Compose evidence');
    const publish = stepByName(job, 'Publish Compose evidence');

    assert.equal(collect.if, 'failure() || cancelled()', `${jobId} diagnostics collection`);
    assert.equal(cleanup.if, 'always()', `${jobId} cleanup`);
    assert.equal(finalize.if, 'always()', `${jobId} finalization`);
    assert.equal(publish.if, 'always()', `${jobId} publication`);
    assert.equal(publish.with['if-no-files-found'], 'error');
    assert.match(publish.with.name, /github\.run_attempt/);
    assert.match(publish.with.path, /compose-stage-evidence-v1\.json/);
    assert.match(publish.with.path, /compose-stage-evidence-v1\.xml/);
  }
});

test('every compose lane resolves a read-only builder digest with cold fallback', () => {
  for (const [jobId, job] of Object.entries(workflow.jobs)) {
    assert.deepEqual(
      job.permissions,
      { contents: 'read', packages: 'read' },
      `${jobId} package permissions`
    );
    const auth = stepByName(job, 'Authenticate to GitHub Container Registry');
    assert.equal(auth.env.BPANE_GHCR_TOKEN, '${{ github.token }}');
    assert.match(auth.run, /BPANE_GHCR_TOKEN/);
    assert.doesNotMatch(auth.run, /github\.token/);
    assert.match(
      stepByName(job, 'Resolve CI Rust builder').run,
      /ci-rust-builder-resolver\.mjs --github-env/
    );
    assert.match(stepByName(job, 'Clean compose resources').run, /docker logout ghcr\.io/);
    assert.doesNotMatch(JSON.stringify(job), /secrets\./);
  }
});

test('workflow initializes and records the checked lane plans for all five lanes', () => {
  const catalog = new ComposeLanePlanCatalog();
  assert.deepEqual(catalog.lanes(), [
    'gateway-default',
    'gateway-docker-pool',
    'browser-integrations',
    'admin-unified',
    'admin-compatibility',
  ]);

  const laneJobs = [
    ['gateway-api', 'gateway-${{ matrix.suite }}'],
    ['browser-integrations', 'browser-integrations'],
    ['admin-promotion', 'admin-${{ matrix.surface }}'],
  ];
  for (const [jobId, lane] of laneJobs) {
    const job = workflow.jobs[jobId];
    assert.match(stepByName(job, 'Initialize Compose evidence').run,
      new RegExp(`--lane ${lane.replaceAll('$', '\\$').replaceAll('{', '\\{').replaceAll('}', '\\}')}`));
    for (const step of steps(job).filter((candidate) => candidate.run?.includes('--stage '))) {
      assert.match(step.run, /node scripts\/compose-evidence\.mjs run/);
    }
  }
});
