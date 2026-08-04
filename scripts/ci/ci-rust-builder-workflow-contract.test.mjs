import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';

import { MATERIAL_INPUTS } from './ci-rust-builder-ref.mjs';
import { YamlDocumentParser } from '../validation/yaml-document-parser.mjs';

const root = path.resolve(import.meta.dirname, '../..');
const workflow = new YamlDocumentParser().parse(
  path.join(root, '.github/workflows/ci-rust-builder.yml')
);

function step(job, name) {
  return job.steps.find((candidate) => candidate.name === name);
}

test('builder workflow watches every material image input', () => {
  for (const trigger of ['pull_request', 'push']) {
    const paths = workflow.on[trigger].paths;
    for (const input of MATERIAL_INPUTS) {
      assert.ok(paths.includes(input), `${trigger} is missing ${input}`);
    }
  }
  assert.deepEqual(workflow.on.push.branches, ['main']);
  assert.ok(workflow.on.workflow_dispatch !== undefined);
});

test('pull requests build and inspect but cannot publish', () => {
  const job = workflow.jobs.validate;
  assert.equal(job.if, "github.event_name == 'pull_request'");
  assert.equal(job.permissions, undefined);
  assert.match(step(job, 'Build builder image without publication').run, /--load/);
  assert.doesNotMatch(JSON.stringify(job), /docker login|--push|packages/);
  assert.match(step(job, 'Verify builder image contract').run, /inspect-ci-rust-builder\.sh/);
});

test('trusted publisher uses bounded package permission and immutable content tags', () => {
  const job = workflow.jobs.publish;
  assert.equal(job.if, "github.event_name != 'pull_request'");
  assert.deepEqual(job.permissions, { contents: 'read', packages: 'write' });
  assert.match(step(job, 'Authenticate to GitHub Container Registry').run, /github\.token/);
  assert.doesNotMatch(JSON.stringify(job), /secrets\./);
  assert.match(step(job, 'Check immutable content tag').run, /imagetools inspect/);

  const publish = step(job, 'Build and publish builder image');
  assert.equal(publish.if, "steps.existing.outputs.published != 'true'");
  assert.match(publish.run, /--provenance=mode=max/);
  assert.match(publish.run, /--sbom=true/);
  assert.match(publish.run, /--push/);
});
