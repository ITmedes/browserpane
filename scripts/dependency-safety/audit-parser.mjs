export class DependencyAuditParser {
  static parseCargo(report, manifest = 'Cargo.lock') {
    const vulnerabilities = report?.vulnerabilities?.list;
    if (!Array.isArray(vulnerabilities)) {
      throw new Error('cargo audit returned an unsupported JSON report');
    }

    return vulnerabilities.map((item) => ({
      ecosystem: 'cargo',
      manifest,
      package: this.#requiredString(item?.package?.name, 'cargo package'),
      advisory: this.#requiredString(item?.advisory?.id, 'cargo advisory'),
      severity: 'vulnerability',
      title: this.#requiredString(item?.advisory?.title, 'cargo advisory title')
    }));
  }

  static parseNpm(report, manifest) {
    if (report?.error) {
      const summary = report.error.summary ?? report.error.message ?? 'unknown error';
      throw new Error(`npm audit failed for ${manifest}: ${summary}`);
    }
    if (!report?.vulnerabilities || typeof report.vulnerabilities !== 'object') {
      throw new Error(`npm audit returned an unsupported JSON report for ${manifest}`);
    }

    const findings = [];
    for (const [packageName, vulnerability] of Object.entries(report.vulnerabilities)) {
      if (!this.#isBlockingNpmSeverity(vulnerability?.severity)) {
        continue;
      }

      const advisoryDetails = this.#resolveNpmAdvisories(
        packageName,
        report.vulnerabilities,
        new Set()
      );
      if (advisoryDetails.length === 0) {
        findings.push({
          ecosystem: 'npm',
          manifest,
          package: packageName,
          advisory: `NPM-PACKAGE-${packageName}`,
          severity: vulnerability.severity,
          title: `Blocking npm audit finding for ${packageName}`
        });
        continue;
      }

      for (const advisory of advisoryDetails) {
        findings.push({
          ecosystem: 'npm',
          manifest,
          package: advisory.name ?? packageName,
          advisory: this.#npmAdvisoryId(advisory),
          severity: advisory.severity,
          title: advisory.title ?? `npm audit finding for ${packageName}`
        });
      }
    }

    return this.#deduplicate(findings);
  }

  static #resolveNpmAdvisories(packageName, vulnerabilities, visited) {
    if (visited.has(packageName)) return [];
    visited.add(packageName);
    const vulnerability = vulnerabilities[packageName];
    if (!vulnerability) return [];

    const details = [];
    for (const entry of vulnerability.via ?? []) {
      if (typeof entry === 'object' && this.#isBlockingNpmSeverity(entry.severity)) {
        details.push(entry);
      } else if (typeof entry === 'string') {
        details.push(...this.#resolveNpmAdvisories(entry, vulnerabilities, visited));
      }
    }
    return details;
  }

  static #deduplicate(findings) {
    const unique = new Map();
    for (const finding of findings) {
      unique.set(this.findingKey(finding), finding);
    }
    return [...unique.values()];
  }

  static findingKey(finding) {
    return [finding.ecosystem, finding.manifest, finding.package, finding.advisory].join('|');
  }

  static #isBlockingNpmSeverity(severity) {
    return severity === 'critical' || severity === 'high';
  }

  static #npmAdvisoryId(advisory) {
    const ghsaId = String(advisory.url ?? '').match(/GHSA-[0-9a-z-]+/i)?.[0];
    if (ghsaId) {
      return ghsaId.toUpperCase();
    }
    return `NPM-${this.#requiredString(advisory.source, 'npm advisory source')}`;
  }

  static #requiredString(value, label) {
    if (typeof value !== 'string' && typeof value !== 'number') {
      throw new Error(`missing ${label}`);
    }
    const normalized = String(value).trim();
    if (normalized.length === 0) {
      throw new Error(`missing ${label}`);
    }
    return normalized;
  }
}
