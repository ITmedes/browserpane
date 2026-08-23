const escapeXml = (value) => String(value)
  .replaceAll('&', '&amp;')
  .replaceAll('<', '&lt;')
  .replaceAll('>', '&gt;')
  .replaceAll('"', '&quot;')
  .replaceAll("'", '&apos;');

export class ComposeJunitWriter {
  write(summary) {
    const evidenceFailure = summary.evidence_errors.length > 0 ? 1 : 0;
    const failures = summary.stages.filter((stage) =>
      ['failure', 'cancelled', 'unknown'].includes(stage.outcome)).length + evidenceFailure;
    const skipped = summary.stages.filter((stage) => stage.outcome === 'skipped').length;
    const cases = summary.stages.map((stage) => this.#stageCase(summary.lane, stage));
    if (evidenceFailure) cases.push(this.#evidenceCase(summary));
    return [
      '<?xml version="1.0" encoding="UTF-8"?>',
      `<testsuites name="BrowserPane Compose evidence" tests="${cases.length}" failures="${failures}" skipped="${skipped}">`,
      `  <testsuite name="${escapeXml(summary.lane)}" tests="${cases.length}" failures="${failures}" skipped="${skipped}">`,
      ...cases,
      '  </testsuite>',
      '</testsuites>',
      '',
    ].join('\n');
  }

  #stageCase(lane, stage) {
    const seconds = ((stage.duration_ms ?? 0) / 1000).toFixed(3);
    const opening = `    <testcase classname="${escapeXml(lane)}" name="${escapeXml(stage.id)}" time="${seconds}">`;
    if (stage.outcome === 'success') return `${opening}</testcase>`;
    if (stage.outcome === 'skipped') return `${opening}<skipped /></testcase>`;
    const failureClass = stage.failure_class ?? 'unknown';
    const message = `${failureClass}:${stage.outcome}`;
    return `${opening}<failure type="${escapeXml(failureClass)}" message="${escapeXml(message)}" /></testcase>`;
  }

  #evidenceCase(summary) {
    const message = summary.evidence_errors.join(',');
    return `    <testcase classname="${escapeXml(summary.lane)}" name="evidence-finalization" time="0.000"><failure type="unknown" message="${escapeXml(message)}" /></testcase>`;
  }
}
