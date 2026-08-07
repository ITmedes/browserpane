import fs from 'node:fs';
import path from 'node:path';

const DEFAULT_REQUIRED_PATHS = [
  'AGENTS.md',
  'ARCH.md',
  'README.md',
  '.github/workflows/validation.yml',
  '.github/workflows/compose.yml',
  '.nvmrc',
  'openapi/bpane-control-v1.yaml',
  'openapi/bpane-control-v1.classifications.json',
  'openapi/bpane-control-v1.examples.json',
  'openapi/bpane-control-v1.operations.json',
  'docs/BPANE-00151_MINIMAL_CI_VALIDATION_PLAN.md',
  'docs/VALIDATION_MATRIX.md',
  'quality/coverage-baselines.json',
  'scripts/check-coverage-baseline.mjs',
  'scripts/check-dependency-safety.mjs',
  'scripts/check-repository-documents.mjs',
  'scripts/collect-compose-diagnostics.mjs',
  'scripts/ci/cleanup-compose.sh',
  'scripts/run-rust-coverage.mjs',
  'scripts/openapi/package-lock.json',
  'scripts/openapi/package.json',
  'scripts/openapi/redocly.yaml',
  'scripts/validate.mjs',
  'rust-toolchain.toml',
  'security/dependency-exceptions.json',
  'security/dependency-exceptions.schema.json'
];

const DEFAULT_PACKAGES = [
  ['code/web/bpane-admin', ['check', 'test', 'build']],
  ['code/web/bpane-admin-unified', ['check', 'test', 'test:coverage', 'build']],
  ['code/web/bpane-client', ['check', 'test', 'test:coverage', 'build']],
  ['code/integrations/mcp-bridge', ['test', 'build']],
  ['code/integrations/recording-worker', ['build']],
  ['code/integrations/workflow-worker', ['build']],
  ['scripts/openapi', ['test', 'check', 'compatibility']]
];

const DEFAULT_DOCUMENTED_COMMANDS = [
  ['AGENTS.md', 'node scripts/check-dependency-safety.mjs'],
  ['AGENTS.md', 'node scripts/check-repository-documents.mjs'],
  ['AGENTS.md', 'node scripts/validate.mjs --profile fast'],
  ['README.md', 'node scripts/check-dependency-safety.mjs'],
  ['README.md', 'node scripts/check-repository-documents.mjs'],
  ['README.md', 'node scripts/validate.mjs --profile fast'],
  ['docs/VALIDATION_MATRIX.md', 'node scripts/validate.mjs --profile fast'],
  ['AGENTS.md', 'npm run check --prefix scripts/openapi'],
  ['README.md', 'npm run check --prefix scripts/openapi'],
  ['docs/VALIDATION_MATRIX.md', 'npm run check --prefix scripts/openapi']
];

export class RepositoryBaselineChecker {
  #rootDirectory;
  #requiredPaths;
  #packages;
  #documentedCommands;

  constructor(rootDirectory, options = {}) {
    this.#rootDirectory = rootDirectory;
    this.#requiredPaths = options.requiredPaths ?? DEFAULT_REQUIRED_PATHS;
    this.#packages = options.packages ?? DEFAULT_PACKAGES;
    this.#documentedCommands = options.documentedCommands ?? DEFAULT_DOCUMENTED_COMMANDS;
  }

  check(trackedJsonFiles) {
    const errors = [];
    this.#checkRequiredPaths(errors);
    this.#checkPackages(errors);
    this.#checkDocumentedCommands(errors);
    this.#checkJsonFiles(trackedJsonFiles, errors);
    return errors;
  }

  #checkRequiredPaths(errors) {
    for (const relativePath of this.#requiredPaths) {
      if (!fs.existsSync(this.#absolute(relativePath))) {
        errors.push(`required path is missing: ${relativePath}`);
      }
    }
  }

  #checkPackages(errors) {
    for (const [directory, requiredScripts] of this.#packages) {
      const manifestPath = `${directory}/package.json`;
      const lockfilePath = `${directory}/package-lock.json`;
      const manifest = this.#readJson(manifestPath, errors);
      const lockfile = this.#readJson(lockfilePath, errors);
      if (!manifest || !lockfile) continue;

      for (const script of requiredScripts) {
        if (typeof manifest.scripts?.[script] !== 'string') {
          errors.push(`${manifestPath} is missing script: ${script}`);
        }
      }
      const lockName = lockfile.packages?.['']?.name ?? lockfile.name;
      if (manifest.name !== lockName) {
        errors.push(`${lockfilePath} root package does not match ${manifest.name}`);
      }
    }
  }

  #checkDocumentedCommands(errors) {
    for (const [relativePath, command] of this.#documentedCommands) {
      const content = this.#readText(relativePath, errors);
      if (content !== null && !content.includes(command)) {
        errors.push(`${relativePath} does not document: ${command}`);
      }
    }
  }

  #checkJsonFiles(trackedJsonFiles, errors) {
    for (const relativePath of trackedJsonFiles) {
      this.#readJson(relativePath, errors);
    }
  }

  #readJson(relativePath, errors) {
    const content = this.#readText(relativePath, errors);
    if (content === null) return null;
    try {
      return JSON.parse(content);
    } catch (error) {
      errors.push(`invalid JSON in ${relativePath}: ${error.message}`);
      return null;
    }
  }

  #readText(relativePath, errors) {
    try {
      return fs.readFileSync(this.#absolute(relativePath), 'utf8');
    } catch (error) {
      errors.push(`unable to read ${relativePath}: ${error.message}`);
      return null;
    }
  }

  #absolute(relativePath) {
    return path.join(this.#rootDirectory, relativePath);
  }
}
