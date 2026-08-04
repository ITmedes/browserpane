import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

export const DEFAULT_REGISTRY = 'ghcr.io/itmedes/browserpane-ci-rust';
export const MATERIAL_INPUTS = Object.freeze([
  'deploy/Dockerfile.ci-rust',
  'deploy/install-rust-toolchain.sh',
  'rust-toolchain.toml',
  'Cargo.lock',
  'Cargo.toml',
  'code/shared/bpane-protocol/Cargo.toml',
  'code/apps/bpane-host/Cargo.toml',
  'code/apps/bpane-gateway/Cargo.toml',
]);

const FORMATS = new Set(['image', 'json', 'tag']);
const REGISTRY_PATTERN = /^[a-z0-9.-]+(?::[0-9]+)?\/[a-z0-9._/-]+$/;

export async function describeCiRustBuilder({
  rootDirectory,
  registry = DEFAULT_REGISTRY,
  platform = 'linux-amd64',
} = {}) {
  if (!rootDirectory) throw new Error('rootDirectory is required');
  if (!REGISTRY_PATTERN.test(registry)) {
    throw new Error(`invalid lowercase container registry reference: ${registry}`);
  }
  if (!/^[a-z0-9][a-z0-9.-]*$/.test(platform)) {
    throw new Error(`invalid platform tag component: ${platform}`);
  }

  const hash = crypto.createHash('sha256');
  let toolchain = '';
  for (const relativePath of MATERIAL_INPUTS) {
    const content = await fs.readFile(path.join(rootDirectory, relativePath));
    hash.update(`${relativePath}\0`);
    hash.update(content);
    hash.update('\0');
    if (relativePath === 'rust-toolchain.toml') {
      toolchain = parseToolchain(content.toString('utf8'));
    }
  }
  const digest = hash.digest('hex');
  const tag = `${platform}-rust-${toolchain}-${digest.slice(0, 24)}`;
  return {
    digest,
    image: `${registry}:${tag}`,
    inputs: [...MATERIAL_INPUTS],
    platform,
    registry,
    tag,
    toolchain,
  };
}

export function parseToolchain(content) {
  const match = /^\s*channel\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"\s*$/m.exec(content);
  if (!match) throw new Error('rust-toolchain.toml must pin an exact numeric channel');
  return match[1];
}

export function parseArguments(argv) {
  const options = {
    format: 'image',
    registry: DEFAULT_REGISTRY,
    rootDirectory: path.resolve(import.meta.dirname, '../..'),
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (argument === '--format' && value) {
      if (!FORMATS.has(value)) throw new Error(`unsupported format: ${value}`);
      options.format = value;
      index += 1;
    } else if (argument === '--registry' && value) {
      options.registry = value;
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
  const description = await describeCiRustBuilder(options);
  if (options.format === 'json') {
    process.stdout.write(`${JSON.stringify(description, null, 2)}\n`);
  } else {
    process.stdout.write(`${description[options.format]}\n`);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main().catch((error) => {
    process.stderr.write(`ci-rust-builder-ref: ${error.message}\n`);
    process.exitCode = 1;
  });
}
