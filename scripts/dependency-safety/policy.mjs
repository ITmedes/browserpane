import path from 'node:path';

import { DependencyAuditParser } from './audit-parser.mjs';

const REQUIRED_FIELDS = [
  'ecosystem',
  'manifest',
  'package',
  'advisory',
  'dependencyPath',
  'runtimeReachability',
  'compensatingControl',
  'reason',
  'owner',
  'expiresOn'
];
const ALLOWED_FIELDS = new Set(REQUIRED_FIELDS);

export class DependencyExceptionPolicy {
  #exceptions;
  #today;

  constructor(document, now = new Date()) {
    this.#today = this.#dateOnly(now);
    this.#exceptions = this.#validate(document);
  }

  evaluate(findings) {
    const exceptionByKey = new Map(
      this.#exceptions.map((exception) => [DependencyAuditParser.findingKey(exception), exception])
    );
    const findingKeys = new Set(findings.map((finding) => DependencyAuditParser.findingKey(finding)));
    const approved = [];
    const blocked = [];

    for (const finding of findings) {
      const exception = exceptionByKey.get(DependencyAuditParser.findingKey(finding));
      if (!exception) {
        blocked.push({ finding, reason: 'no approved exception' });
      } else if (this.#isExpired(exception)) {
        blocked.push({ finding, reason: `exception expired on ${exception.expiresOn}` });
      } else {
        approved.push({ finding, exception });
      }
    }

    const stale = this.#exceptions.filter(
      (exception) => !findingKeys.has(DependencyAuditParser.findingKey(exception))
    );
    const expired = this.#exceptions.filter((exception) => this.#isExpired(exception));
    return {
      passed: blocked.length === 0 && stale.length === 0 && expired.length === 0,
      approved,
      blocked,
      stale,
      expired
    };
  }

  #validate(document) {
    const errors = [];
    if (!document || typeof document !== 'object' || Array.isArray(document)) {
      throw new Error('dependency exception document must be an object');
    }
    const topLevelFields = new Set(['$schema', 'version', 'exceptions']);
    for (const field of Object.keys(document)) {
      if (!topLevelFields.has(field)) errors.push(`unsupported top-level field: ${field}`);
    }
    if (document.version !== 1) errors.push('version must be 1');
    if (!Array.isArray(document.exceptions)) errors.push('exceptions must be an array');

    const exceptions = Array.isArray(document.exceptions) ? document.exceptions : [];
    const keys = new Set();
    for (const [index, exception] of exceptions.entries()) {
      const prefix = `exceptions[${index}]`;
      if (!exception || typeof exception !== 'object' || Array.isArray(exception)) {
        errors.push(`${prefix} must be an object`);
        continue;
      }
      for (const field of Object.keys(exception)) {
        if (!ALLOWED_FIELDS.has(field)) errors.push(`${prefix} has unsupported field: ${field}`);
      }
      for (const field of REQUIRED_FIELDS) {
        if (typeof exception[field] !== 'string' || exception[field].trim().length === 0) {
          errors.push(`${prefix}.${field} must be a non-empty string`);
        }
      }
      if (!['cargo', 'npm'].includes(exception.ecosystem)) {
        errors.push(`${prefix}.ecosystem must be cargo or npm`);
      }
      if (typeof exception.manifest === 'string' && this.#isUnsafeManifest(exception.manifest)) {
        errors.push(`${prefix}.manifest must be a repository-relative lockfile path`);
      }
      if (typeof exception.expiresOn === 'string' && !this.#isDate(exception.expiresOn)) {
        errors.push(`${prefix}.expiresOn must be a valid YYYY-MM-DD date`);
      }
      const key = DependencyAuditParser.findingKey(exception);
      if (keys.has(key)) errors.push(`${prefix} duplicates ${key}`);
      keys.add(key);
    }

    if (errors.length > 0) {
      throw new Error(`invalid dependency exceptions:\n- ${errors.join('\n- ')}`);
    }
    return exceptions;
  }

  #isUnsafeManifest(manifest) {
    const normalized = path.posix.normalize(manifest);
    const isNpmLockfile = normalized === 'package-lock.json'
      || normalized.endsWith('/package-lock.json');
    return path.posix.isAbsolute(manifest)
      || path.win32.isAbsolute(manifest)
      || normalized.startsWith('../')
      || (normalized !== 'Cargo.lock' && !isNpmLockfile);
  }

  #isDate(value) {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
    const parsed = new Date(`${value}T00:00:00.000Z`);
    return !Number.isNaN(parsed.valueOf()) && this.#dateOnly(parsed) === value;
  }

  #isExpired(exception) {
    return exception.expiresOn < this.#today;
  }

  #dateOnly(date) {
    if (!(date instanceof Date) || Number.isNaN(date.valueOf())) {
      throw new Error('policy clock must be a valid Date');
    }
    return date.toISOString().slice(0, 10);
  }
}
