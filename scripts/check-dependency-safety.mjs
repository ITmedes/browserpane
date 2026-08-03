#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { DependencyAuditRunner } from './dependency-safety/audit-runner.mjs';
import { DependencyExceptionPolicy } from './dependency-safety/policy.mjs';

class DependencySafetyCommand {
  #rootDirectory;

  constructor() {
    this.#rootDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  }

  run(args) {
    if (args.includes('--help') || args.includes('-h')) {
      this.#printHelp();
      return 0;
    }
    if (args.length > 0) {
      throw new Error(`unknown argument: ${args[0]}`);
    }

    const exceptionsPath = path.join(this.#rootDirectory, 'security/dependency-exceptions.json');
    const document = JSON.parse(fs.readFileSync(exceptionsPath, 'utf8'));
    const policy = new DependencyExceptionPolicy(document);
    const audit = new DependencyAuditRunner(this.#rootDirectory).run();
    const result = policy.evaluate(audit.findings);

    console.log(`Scanned Cargo.lock and ${audit.npmLockfiles.length} npm lockfiles.`);
    for (const item of result.approved) {
      console.log(`APPROVED ${this.#formatFinding(item.finding)} until ${item.exception.expiresOn}`);
    }
    for (const item of result.blocked) {
      console.error(`BLOCKED ${this.#formatFinding(item.finding)}: ${item.reason}`);
    }
    for (const exception of result.stale) {
      console.error(`STALE ${this.#formatFinding(exception)}: remove or reconcile the exception`);
    }
    for (const exception of result.expired) {
      console.error(`EXPIRED ${this.#formatFinding(exception)} on ${exception.expiresOn}`);
    }
    if (!result.passed) return 1;

    console.log('Dependency safety policy passed.');
    return 0;
  }

  #formatFinding(finding) {
    return `${finding.ecosystem}:${finding.package}:${finding.advisory} (${finding.manifest})`;
  }

  #printHelp() {
    console.log(`Usage: node scripts/check-dependency-safety.mjs

Scans Cargo.lock with cargo-audit and every committed npm package-lock.json
with npm audit. RustSec vulnerabilities and npm critical/high findings fail
unless security/dependency-exceptions.json contains an exact, unexpired match.
Stale and expired exceptions also fail.

Prerequisite: cargo install cargo-audit --locked`);
  }
}

try {
  process.exitCode = new DependencySafetyCommand().run(process.argv.slice(2));
} catch (error) {
  console.error(`Dependency safety check failed: ${error.message}`);
  process.exitCode = 2;
}
