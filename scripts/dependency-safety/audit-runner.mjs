import { spawnSync } from 'node:child_process';
import path from 'node:path';

import { DependencyAuditParser } from './audit-parser.mjs';

export class DependencyAuditRunner {
  #rootDirectory;
  #execute;

  constructor(rootDirectory, execute = spawnSync) {
    this.#rootDirectory = rootDirectory;
    this.#execute = execute;
  }

  run() {
    this.#requireCargoAudit();
    const npmLockfiles = this.#npmLockfiles();
    const findings = this.#cargoFindings();
    for (const lockfile of npmLockfiles) {
      findings.push(...this.#npmFindings(lockfile));
    }
    return { findings, npmLockfiles };
  }

  #requireCargoAudit() {
    const result = this.#runCommand('cargo', ['audit', '--version'], this.#rootDirectory);
    if (result.status !== 0) {
      throw new Error('cargo-audit is required; install it with `cargo install cargo-audit --locked`');
    }
  }

  #npmLockfiles() {
    const result = this.#runCommand(
      'git',
      ['ls-files', '--cached', '--', '**/package-lock.json'],
      this.#rootDirectory
    );
    if (result.status !== 0) {
      throw new Error(`unable to list committed npm lockfiles: ${this.#commandError(result)}`);
    }
    const lockfiles = result.stdout.split('\n').map((value) => value.trim()).filter(Boolean);
    if (lockfiles.length === 0) {
      throw new Error('no committed npm package-lock.json files were found');
    }
    return lockfiles.sort();
  }

  #cargoFindings() {
    const result = this.#runCommand('cargo', ['audit', '--json'], this.#rootDirectory);
    const report = this.#parseJson(result, 'cargo audit');
    return DependencyAuditParser.parseCargo(report);
  }

  #npmFindings(lockfile) {
    const workingDirectory = path.join(this.#rootDirectory, path.dirname(lockfile));
    const result = this.#runCommand(
      'npm',
      ['audit', '--package-lock-only', '--json'],
      workingDirectory
    );
    const report = this.#parseJson(result, `npm audit for ${lockfile}`);
    return DependencyAuditParser.parseNpm(report, lockfile);
  }

  #runCommand(command, args, cwd) {
    const result = this.#execute(command, args, {
      cwd,
      encoding: 'utf8',
      maxBuffer: 30 * 1024 * 1024
    });
    if (result.error) {
      throw new Error(`unable to execute ${command}: ${result.error.message}`);
    }
    return result;
  }

  #parseJson(result, label) {
    try {
      return JSON.parse(result.stdout);
    } catch {
      throw new Error(`${label} did not return JSON: ${this.#commandError(result)}`);
    }
  }

  #commandError(result) {
    return String(result.stderr || result.stdout || `exit ${result.status}`).trim().slice(0, 500);
  }
}
