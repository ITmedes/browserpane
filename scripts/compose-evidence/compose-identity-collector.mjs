import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

import { COMPOSE_TEST_PLAN_FILES } from './compose-lane-plans.mjs';

const ZERO_GIT_ID = '0'.repeat(40);
const ZERO_SHA256 = '0'.repeat(64);
const DIGEST_PATTERN = /sha256:[0-9a-f]{64}/g;

export class ComposeIdentityCollector {
  #rootDirectory;
  #execute;

  constructor(rootDirectory, execute = execFileSync) {
    this.#rootDirectory = rootDirectory;
    this.#execute = execute;
  }

  collect(lane) {
    const errors = [];
    const testedCommit = this.#git(['rev-parse', 'HEAD'], 'commit', errors);
    const testedTree = this.#git(['rev-parse', 'HEAD^{tree}'], 'tree', errors);
    const workflowSha = this.#hashFiles(['.github/workflows/compose.yml'], errors);
    const testPlanSha = this.#hashFiles(COMPOSE_TEST_PLAN_FILES, errors);
    const imageDigests = this.#imageDigests(lane, errors);
    return {
      identity: {
        tested_commit: testedCommit ?? ZERO_GIT_ID,
        tested_tree: testedTree ?? ZERO_GIT_ID,
        workflow_sha256: workflowSha ?? ZERO_SHA256,
        test_plan_sha256: testPlanSha ?? ZERO_SHA256,
        image_digests: imageDigests,
      },
      errors,
    };
  }

  #git(args, name, errors) {
    try {
      return String(this.#execute('git', args, this.#options())).trim();
    } catch {
      errors.push(`identity-${name}-unavailable`);
      return null;
    }
  }

  #hashFiles(relativePaths, errors) {
    const hash = crypto.createHash('sha256');
    try {
      for (const relativePath of [...relativePaths].sort()) {
        hash.update(relativePath);
        hash.update('\0');
        hash.update(fs.readFileSync(path.join(this.#rootDirectory, relativePath)));
        hash.update('\0');
      }
      return hash.digest('hex');
    } catch {
      errors.push('identity-test-plan-unavailable');
      return null;
    }
  }

  #imageDigests(lane, errors) {
    const composeFiles = [['-f', 'deploy/compose.yml', '--profile', 'workflow']];
    if (lane === 'admin-compatibility') {
      composeFiles.push(['-f', 'deploy/examples/egress-observer/compose.yml']);
      composeFiles.push(['-f', 'deploy/examples/egress-observer/compose.tls.yml']);
    }
    const imageNames = [];
    for (const composeArgs of composeFiles) {
      try {
        const output = String(this.#execute('docker',
          ['compose', ...composeArgs, 'config', '--images'], this.#options()));
        imageNames.push(...output.split(/\r?\n/).filter(Boolean));
      } catch {
        errors.push('identity-image-plan-unavailable');
      }
    }
    const digests = new Set(imageNames.flatMap((name) => name.match(DIGEST_PATTERN) ?? []));
    for (const [index, imageName] of [...new Set(imageNames)].entries()) {
      try {
        const output = String(this.#execute('docker',
          ['image', 'inspect', '--format', '{{json .RepoDigests}} {{.Id}}', imageName],
          this.#options()));
        for (const digest of output.match(DIGEST_PATTERN) ?? []) digests.add(digest);
      } catch {
        errors.push(`identity-image-${index + 1}-unavailable`);
      }
    }
    return [...digests].sort().slice(0, 64);
  }

  #options() {
    return { cwd: this.#rootDirectory, encoding: 'utf8', timeout: 10_000, maxBuffer: 1_048_576 };
  }
}
