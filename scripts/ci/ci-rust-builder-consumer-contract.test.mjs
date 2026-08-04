import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

import { YamlDocumentParser } from '../validation/yaml-document-parser.mjs';

const root = path.resolve(import.meta.dirname, '../..');
const fallback = 'ubuntu:24.04@sha256:561618e2c15bf2397621dd04f96926663a3b5616c189cf7e38db7e82f5c538ea';

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), 'utf8');
}

test('Rust service and test Dockerfiles retain the pinned cold fallback', () => {
  for (const relativePath of [
    'deploy/Dockerfile.gateway',
    'deploy/Dockerfile.host',
    'deploy/Dockerfile.test',
  ]) {
    const dockerfile = read(relativePath);
    assert.match(
      dockerfile,
      new RegExp(`ARG BPANE_RUST_BUILDER_IMAGE=${fallback.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`),
      relativePath
    );
    assert.match(dockerfile, /FROM \$\{BPANE_RUST_BUILDER_IMAGE\}/, relativePath);
    assert.match(dockerfile, /COPY deploy\/install-rust-toolchain\.sh rust-toolchain\.toml \/tmp\//, relativePath);
    assert.match(dockerfile, /rust_toolchain=.*channel/, relativePath);
    assert.match(dockerfile, /install-rust-toolchain\.sh "\$\{rust_toolchain\}"/, relativePath);
    assert.match(dockerfile, /--locked/, relativePath);
  }
});

test('compose passes the optional builder reference only to Rust services', () => {
  const compose = new YamlDocumentParser().parse(path.join(root, 'deploy/compose.yml'));
  for (const service of ['gateway', 'host']) {
    assert.equal(
      compose.services[service].build.args.BPANE_RUST_BUILDER_IMAGE,
      `\${BPANE_RUST_BUILDER_IMAGE:-${fallback}}`
    );
  }
  assert.equal(compose.services.web.build.args, undefined);
});

test('pinned Rust installer supports hosted and local Docker architectures', () => {
  const installer = read('deploy/install-rust-toolchain.sh');
  assert.match(installer, /amd64\)/);
  assert.match(installer, /arm64\)/);
  assert.match(installer, /rustup-init.*sha256sum/s);
  assert.match(installer, /--default-toolchain none/);
  assert.match(installer, /auto-self-update disable/);
  assert.match(installer, /\^\[0-9\]\+\\\.\[0-9\]\+\\\.\[0-9\]\+\$/);

  const syntax = spawnSync('sh', ['-n', path.join(root, 'deploy/install-rust-toolchain.sh')]);
  assert.equal(syntax.status, 0, syntax.stderr?.toString());
  for (const invalidVersion of ['stable', '1.93', '1.93.1-x86_64', '1..93']) {
    const result = spawnSync(
      path.join(root, 'deploy/install-rust-toolchain.sh'),
      [invalidVersion],
      { encoding: 'utf8' }
    );
    assert.equal(result.status, 2, invalidVersion);
    assert.match(result.stderr, /exact numeric version/, invalidVersion);
  }
});
