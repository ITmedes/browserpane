import assert from 'node:assert/strict';
import test from 'node:test';

import {
  CiRustBuilderResolver,
  parseInspect,
  selectDigestReference,
  validateMetadata,
} from './ci-rust-builder-resolver.mjs';

const description = {
  image: 'ghcr.io/itmedes/browserpane-ci-rust:linux-amd64-rust-1.93.1-example',
  registry: 'ghcr.io/itmedes/browserpane-ci-rust',
  tag: 'linux-amd64-rust-1.93.1-example',
};
const digestReference = `${description.registry}@sha256:${'a'.repeat(64)}`;

function metadata(overrides = {}) {
  return {
    Architecture: 'amd64',
    Config: {
      Labels: {
        'org.opencontainers.image.source': 'https://github.com/ITmedes/browserpane',
        'org.opencontainers.image.version': description.tag,
      },
    },
    Os: 'linux',
    RepoDigests: [digestReference],
    ...overrides,
  };
}

test('resolver returns an immutable digest after a valid pull', () => {
  const commands = [];
  const resolver = new CiRustBuilderResolver({
    run(command, args) {
      commands.push([command, args]);
      if (args[0] === 'pull') return { exitCode: 0, stderr: '', stdout: '' };
      return { exitCode: 0, stderr: '', stdout: JSON.stringify([metadata()]) };
    },
  });

  const result = resolver.resolve(description);

  assert.equal(result.status, 'hit');
  assert.equal(result.digestReference, digestReference);
  assert.deepEqual(commands, [
    ['docker', ['pull', description.image]],
    ['docker', ['image', 'inspect', description.image]],
  ]);
});

test('resolver treats an unavailable package as an explicit cold fallback', () => {
  const resolver = new CiRustBuilderResolver({
    run: () => ({ exitCode: 1, stderr: 'denied', stdout: '' }),
  });

  assert.deepEqual(
    { ...resolver.resolve(description), elapsedMs: 0 },
    { elapsedMs: 0, image: description.image, reason: 'pull_failed', status: 'fallback' }
  );
});

test('resolver rejects malformed or untrusted pulled images', () => {
  for (const invalid of [
    metadata({ Architecture: 'arm64' }),
    metadata({ Config: { Labels: {} } }),
    metadata({ RepoDigests: [] }),
  ]) {
    const resolver = new CiRustBuilderResolver({
      run(_command, args) {
        if (args[0] === 'pull') return { exitCode: 0, stderr: '', stdout: '' };
        return { exitCode: 0, stderr: '', stdout: JSON.stringify([invalid]) };
      },
    });
    assert.throws(() => resolver.resolve(description));
  }
});

test('inspection helpers reject invalid JSON, labels, and digest shapes', () => {
  assert.throws(() => parseInspect('not-json'), /invalid JSON/);
  assert.throws(() => parseInspect('[]'), /no image metadata/);
  assert.throws(
    () => validateMetadata(metadata({ Os: 'windows' }), description),
    /platform mismatch/
  );
  assert.throws(
    () => selectDigestReference([`${description.registry}@sha256:short`], description.registry),
    /no valid digest/
  );
});
