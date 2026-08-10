import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';

import {
  COMPATIBILITY_ADMIN_PROMOTION_SMOKES,
  UNIFIED_ADMIN_PROMOTION_SMOKES,
} from '../validation/admin-promotion-contract.mjs';
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
    stepByName(job, 'Prepare compose runtime').run,
    'scripts/run-gateway-compose-e2e.sh --suite stack'
  );
  for (const stage of expectedStages) {
    assert.match(command, new RegExp(`--stage ${stage}(?:\\s|$)`));
  }
  assert.equal((command.match(/--stage /g) ?? []).length, expectedStages.length);
});

test('compose workflow runs unified and compatibility promotion lanes independently', () => {
  const job = workflow.jobs['admin-promotion'];

  assert.ok(job);
  assert.equal(job.strategy['fail-fast'], false);
  assert.deepEqual(job.strategy.matrix.surface, ['unified', 'compatibility']);
  assert.equal(
    stepByName(job, 'Run admin promotion validation').run,
    'node scripts/run-admin-promotion-validation.mjs "${{ matrix.surface }}"',
  );
  assert.ok(UNIFIED_ADMIN_PROMOTION_SMOKES.length > 0);
  assert.ok(COMPATIBILITY_ADMIN_PROMOTION_SMOKES.length > 0);
});

test('every compose lane retains failure diagnostics and unconditional cleanup', () => {
  for (const [jobId, job] of Object.entries(workflow.jobs)) {
    const collect = stepByName(job, 'Collect redacted failure diagnostics');
    const publish = stepByName(job, 'Publish redacted failure diagnostics');
    const cleanup = stepByName(job, 'Clean compose resources');

    assert.equal(collect.if, 'failure()', `${jobId} diagnostics collection`);
    assert.equal(publish.if, 'failure()', `${jobId} diagnostics publication`);
    assert.equal(cleanup.if, 'always()', `${jobId} cleanup`);
  }
});

test('every compose lane resolves a read-only builder digest with cold fallback', () => {
  for (const [jobId, job] of Object.entries(workflow.jobs)) {
    assert.deepEqual(
      job.permissions,
      { contents: 'read', packages: 'read' },
      `${jobId} package permissions`
    );
    assert.match(
      stepByName(job, 'Authenticate to GitHub Container Registry').run,
      /github\.token/
    );
    assert.match(
      stepByName(job, 'Resolve CI Rust builder').run,
      /ci-rust-builder-resolver\.mjs --github-env/
    );
    assert.equal(stepByName(job, 'Log out of GitHub Container Registry').if, 'always()');
    assert.doesNotMatch(JSON.stringify(job), /secrets\./);
  }
});
