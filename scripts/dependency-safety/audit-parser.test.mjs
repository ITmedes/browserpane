import assert from 'node:assert/strict';
import test from 'node:test';

import { DependencyAuditParser } from './audit-parser.mjs';

test('parseCargo maps every RustSec vulnerability to a blocking finding', () => {
  const findings = DependencyAuditParser.parseCargo({
    vulnerabilities: {
      list: [{
        advisory: { id: 'RUSTSEC-2026-0001', title: 'Example vulnerability' },
        package: { name: 'example-crate' }
      }]
    }
  });

  assert.deepEqual(findings, [{
    ecosystem: 'cargo',
    manifest: 'Cargo.lock',
    package: 'example-crate',
    advisory: 'RUSTSEC-2026-0001',
    severity: 'vulnerability',
    title: 'Example vulnerability'
  }]);
});

test('parseNpm keeps critical and high advisories and ignores lower severity', () => {
  const findings = DependencyAuditParser.parseNpm({
    vulnerabilities: {
      dangerous: {
        severity: 'critical',
        via: [{
          source: 42,
          name: 'dangerous',
          severity: 'critical',
          title: 'Critical fixture',
          url: 'https://github.com/advisories/GHSA-aaaa-bbbb-cccc'
        }]
      },
      moderate: {
        severity: 'moderate',
        via: [{ source: 43, severity: 'moderate' }]
      }
    }
  }, 'example/package-lock.json');

  assert.equal(findings.length, 1);
  assert.equal(findings[0].advisory, 'GHSA-AAAA-BBBB-CCCC');
  assert.equal(findings[0].package, 'dangerous');
});

test('parseNpm resolves a blocking advisory through transitive via entries', () => {
  const findings = DependencyAuditParser.parseNpm({
    vulnerabilities: {
      wrapper: { severity: 'critical', via: ['dangerous'] },
      dangerous: {
        severity: 'critical',
        via: [{
          source: 42,
          name: 'dangerous',
          severity: 'critical',
          title: 'Critical fixture',
          url: 'https://github.com/advisories/GHSA-aaaa-bbbb-cccc'
        }]
      }
    }
  }, 'example/package-lock.json');

  assert.equal(findings.length, 1);
  assert.equal(findings[0].package, 'dangerous');
  assert.equal(findings[0].advisory, 'GHSA-AAAA-BBBB-CCCC');
});

test('parseNpm rejects registry errors instead of treating them as a clean scan', () => {
  assert.throws(
    () => DependencyAuditParser.parseNpm(
      { error: { summary: 'registry unavailable' } },
      'example/package-lock.json'
    ),
    /registry unavailable/
  );
});
