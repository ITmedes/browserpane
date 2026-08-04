import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { describeCiRustBuilder } from './ci-rust-builder-ref.mjs';

const DIGEST_PATTERN = /^ghcr\.io\/[a-z0-9._/-]+@sha256:[0-9a-f]{64}$/;
const EXPECTED_SOURCE = 'https://github.com/ITmedes/browserpane';

export class CiRustBuilderResolver {
  #run;

  constructor({ run = runCommand } = {}) {
    this.#run = run;
  }

  resolve(description) {
    const startedAt = Date.now();
    const pull = this.#run('docker', ['pull', description.image]);
    if (pull.exitCode !== 0) {
      return {
        elapsedMs: Date.now() - startedAt,
        image: description.image,
        reason: 'pull_failed',
        status: 'fallback',
      };
    }

    const inspect = this.#run('docker', ['image', 'inspect', description.image]);
    if (inspect.exitCode !== 0) {
      throw new Error(`builder image inspection failed: ${inspect.stderr.trim()}`);
    }
    const metadata = parseInspect(inspect.stdout);
    validateMetadata(metadata, description);
    const digestReference = selectDigestReference(metadata.RepoDigests, description.registry);

    return {
      digestReference,
      elapsedMs: Date.now() - startedAt,
      image: description.image,
      status: 'hit',
    };
  }
}

export function parseInspect(output) {
  let parsed;
  try {
    parsed = JSON.parse(output);
  } catch (error) {
    throw new Error(`builder image inspection returned invalid JSON: ${error.message}`);
  }
  const metadata = Array.isArray(parsed) ? parsed[0] : parsed;
  if (!metadata || typeof metadata !== 'object') {
    throw new Error('builder image inspection returned no image metadata');
  }
  return metadata;
}

export function validateMetadata(metadata, description) {
  if (metadata.Architecture !== 'amd64' || metadata.Os !== 'linux') {
    throw new Error(
      `builder image platform mismatch: expected linux/amd64, found ${metadata.Os}/${metadata.Architecture}`
    );
  }
  const labels = metadata.Config?.Labels ?? {};
  if (labels['org.opencontainers.image.source'] !== EXPECTED_SOURCE) {
    throw new Error('builder image source label does not match BrowserPane');
  }
  if (labels['org.opencontainers.image.version'] !== description.tag) {
    throw new Error('builder image version label does not match its content tag');
  }
}

export function selectDigestReference(repoDigests, registry) {
  const prefix = `${registry}@sha256:`;
  const candidate = Array.isArray(repoDigests)
    ? repoDigests.find((value) => value.startsWith(prefix))
    : undefined;
  if (!candidate || !DIGEST_PATTERN.test(candidate)) {
    throw new Error(`builder image has no valid digest for ${registry}`);
  }
  return candidate;
}

function runCommand(command, args) {
  const result = spawnSync(command, args, { encoding: 'utf8' });
  if (result.error) {
    return { exitCode: 1, stderr: result.error.message, stdout: '' };
  }
  return {
    exitCode: result.status ?? 1,
    stderr: result.stderr ?? '',
    stdout: result.stdout ?? '',
  };
}

function parseArguments(argv) {
  const options = { githubEnvironment: '', rootDirectory: path.resolve(import.meta.dirname, '../..') };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (argument === '--github-env' && value) {
      options.githubEnvironment = path.resolve(value);
      index += 1;
    } else if (argument === '--root' && value) {
      options.rootDirectory = path.resolve(value);
      index += 1;
    } else {
      throw new Error(`unknown or incomplete argument: ${argument}`);
    }
  }
  return options;
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const description = await describeCiRustBuilder({ rootDirectory: options.rootDirectory });
  const result = new CiRustBuilderResolver().resolve(description);
  if (options.githubEnvironment) {
    const lines = [`BPANE_CI_RUST_BUILDER_STATUS=${result.status}`];
    if (result.digestReference) lines.push(`BPANE_RUST_BUILDER_IMAGE=${result.digestReference}`);
    await fs.appendFile(options.githubEnvironment, `${lines.join('\n')}\n`);
  }
  process.stdout.write(`${JSON.stringify(result)}\n`);
  if (result.status === 'fallback' && process.env.GITHUB_ACTIONS === 'true') {
    process.stdout.write(
      `::warning::CI Rust builder ${result.image} is unavailable; using the cold Ubuntu fallback.\n`
    );
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main().catch((error) => {
    process.stderr.write(`ci-rust-builder-resolver: ${error.message}\n`);
    process.exitCode = 1;
  });
}
