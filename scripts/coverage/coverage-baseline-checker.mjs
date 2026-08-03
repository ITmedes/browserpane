import fs from 'node:fs';
import path from 'node:path';

const DEFAULT_CONFIG_PATH = 'quality/coverage-baselines.json';
const OUTPUT_DIRECTORY = 'test-results/coverage';

export class CoverageBaselineChecker {
  #rootDirectory;
  #configuration;

  constructor(rootDirectory, configurationPath = DEFAULT_CONFIG_PATH) {
    this.#rootDirectory = rootDirectory;
    this.#configuration = this.#readJson(configurationPath);
    if (this.#configuration.version !== 1 || typeof this.#configuration.targets !== 'object') {
      throw new Error(`invalid coverage baseline configuration: ${configurationPath}`);
    }
  }

  check(targetId, options = {}) {
    const target = this.#configuration.targets[targetId];
    if (!target) throw new Error(`unknown coverage target: ${targetId}`);
    const reportPath = options.reportPath ?? target.report;
    const actual = this.#readMetrics(target.format, this.#readJson(reportPath));
    const comparisons = Object.entries(target.minimums).map(([metric, minimum]) => {
      const measured = actual[metric];
      if (!Number.isFinite(minimum) || !Number.isFinite(measured)) {
        throw new Error(`invalid ${metric} coverage for target ${targetId}`);
      }
      return { metric, actual: measured, minimum, passed: measured >= minimum };
    });
    const result = {
      targetId,
      label: target.label,
      reportPath,
      passed: comparisons.every((comparison) => comparison.passed),
      comparisons
    };
    this.#writeSummary(result, options.outputPath);
    if (!result.passed) {
      const failures = comparisons
        .filter((comparison) => !comparison.passed)
        .map((comparison) => `${comparison.metric} ${this.#percent(comparison.actual)} < ${this.#percent(comparison.minimum)}`);
      throw new Error(`coverage baseline failed for ${targetId}: ${failures.join(', ')}`);
    }
    return result;
  }

  #readMetrics(format, report) {
    if (format === 'istanbul-summary') return this.#istanbulMetrics(report);
    if (format === 'llvm-summary') return this.#llvmMetrics(report);
    throw new Error(`unsupported coverage report format: ${format}`);
  }

  #istanbulMetrics(report) {
    const total = report.total;
    if (!total || typeof total !== 'object') throw new Error('Istanbul coverage report has no total');
    return Object.fromEntries(
      ['statements', 'branches', 'functions', 'lines'].map((metric) => [metric, total[metric]?.pct])
    );
  }

  #llvmMetrics(report) {
    const totals = report.data?.[0]?.totals;
    if (!totals || typeof totals !== 'object') throw new Error('LLVM coverage report has no totals');
    return Object.fromEntries(
      ['branches', 'functions', 'lines', 'regions'].map((metric) => [metric, totals[metric]?.percent])
    );
  }

  #writeSummary(result, requestedPath) {
    const outputPath = requestedPath ?? path.join(OUTPUT_DIRECTORY, `${result.targetId}.md`);
    const absolutePath = this.#absolute(outputPath);
    fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
    const rows = result.comparisons.map((comparison) =>
      `| ${comparison.metric} | ${this.#percent(comparison.actual)} | ${this.#percent(comparison.minimum)} | ${comparison.passed ? 'PASS' : 'FAIL'} |`
    );
    const markdown = [
      `# Coverage: ${result.label}`,
      '',
      `Result: **${result.passed ? 'PASS' : 'FAIL'}**`,
      '',
      '| Metric | Actual | Minimum | Status |',
      '| --- | ---: | ---: | --- |',
      ...rows,
      '',
      `Source report: \`${result.reportPath}\``,
      ''
    ].join('\n');
    fs.writeFileSync(absolutePath, markdown);
  }

  #readJson(relativePath) {
    try {
      return JSON.parse(fs.readFileSync(this.#absolute(relativePath), 'utf8'));
    } catch (error) {
      throw new Error(`unable to read coverage JSON ${relativePath}: ${error.message}`);
    }
  }

  #absolute(relativePath) {
    return path.isAbsolute(relativePath) ? relativePath : path.join(this.#rootDirectory, relativePath);
  }

  #percent(value) {
    return `${value.toFixed(2)}%`;
  }
}
