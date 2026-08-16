import fs from 'node:fs';
import path from 'node:path';

import { YamlDocumentParser } from '../validation/yaml-document-parser.mjs';

const EXPECTED_RECORDS = new Set([
  'browserpane:gateway_http_requests:rate5m',
  'browserpane:gateway_http_5xx:ratio_rate5m',
  'browserpane:gateway_http_request_duration_seconds:p95_rate5m',
  'browserpane:runtime_capacity_utilization:ratio',
  'browserpane:workflow_produced_file_upload_failure:ratio_increase15m',
  'browserpane:workflow_event_delivery_success:ratio_increase15m',
  'browserpane:recording_artifact_finalize_success:ratio_increase15m',
  'browserpane:workflow_produced_file_upload_failures:increase15m',
  'browserpane:workflow_event_delivery_retries:increase15m',
  'browserpane:workflow_event_delivery_failures:increase15m',
  'browserpane:recording_artifact_finalize_failures:increase15m',
  'browserpane:recording_failures:increase15m',
  'browserpane:recording_playback_export_failures:increase15m',
  'browserpane:workflow_retention_failures:increase1h',
  'browserpane:recording_retention_failures:increase1h',
]);

const EXPECTED_ALERTS = new Set([
  'BrowserPaneGatewayMetricsUnavailable',
  'BrowserPaneGatewayHighServerErrorRatio',
  'BrowserPaneRuntimeCapacitySaturated',
  'BrowserPaneWorkflowProducedFileUploadFailure',
  'BrowserPaneWorkflowEventDeliveryRetrying',
  'BrowserPaneWorkflowEventDeliveryFailure',
  'BrowserPaneRecordingArtifactFinalizeFailure',
  'BrowserPaneRecordingWorkerFailure',
  'BrowserPaneRecordingPlaybackExportFailure',
  'BrowserPaneRetentionFailure',
]);

const FORBIDDEN_RULE_FRAGMENTS = [
  'owner_id', 'project_id', 'session_id', 'workflow_id', 'recording_id',
  'target_url', 'authorization', 'bearer', 'credential', 'payload',
  'artifact_ref', 'raw_error', '{{',
];

export class PrometheusRulesContract {
  #root;
  #parser;

  constructor(root, parser = new YamlDocumentParser()) {
    this.#root = root;
    this.#parser = parser;
  }

  check() {
    const errors = [];
    const directory = path.join(this.#root, 'deploy/examples/observability');
    const config = this.#parser.parse(path.join(directory, 'prometheus.yml'));
    const recordings = this.#parser.parse(path.join(directory, 'recording-rules.yml'));
    const alerts = this.#parser.parse(path.join(directory, 'alert-rules.yml'));
    const tests = this.#parser.parse(path.join(directory, 'rule-tests.yml'));
    const runbook = fs.readFileSync(
      path.join(this.#root, 'docs/operations/PROMETHEUS_ALERT_RUNBOOK.md'),
      'utf8',
    );

    this.#sameSet(errors, 'Prometheus rule_files', config.rule_files, [
      'recording-rules.yml', 'alert-rules.yml',
    ]);
    const recordingRules = this.#rules(recordings);
    this.#sameSet(errors, 'recording rules', recordingRules.map((rule) => rule.record), EXPECTED_RECORDS);
    for (const rule of recordingRules) {
      if (!rule.expr) errors.push(`recording rule ${rule.record ?? '<unnamed>'} has no expression`);
      if (rule.labels) errors.push(`recording rule ${rule.record} must not add labels`);
    }

    const alertRules = this.#rules(alerts);
    this.#sameSet(errors, 'alert rules', alertRules.map((rule) => rule.alert), EXPECTED_ALERTS);
    for (const rule of alertRules) this.#checkAlert(errors, rule, runbook);

    this.#sameSet(errors, 'rule test files', tests.rule_files, [
      'recording-rules.yml', 'alert-rules.yml',
    ]);
    const testedAlerts = (tests.tests ?? []).flatMap((entry) => (
      entry.alert_rule_test ?? []
    )).map((entry) => entry.alertname);
    for (const alert of EXPECTED_ALERTS) {
      if (!testedAlerts.includes(alert)) errors.push(`alert ${alert} has no semantic rule test`);
    }
    const testNames = (tests.tests ?? []).map((entry) => String(entry.name ?? '').toLowerCase());
    for (const behavior of ['healthy', 'missing', 'recovery', 'reset']) {
      if (!testNames.some((name) => name.includes(behavior))) {
        errors.push(`rule tests do not name a ${behavior} behavior case`);
      }
    }

    const ruleText = [
      fs.readFileSync(path.join(directory, 'recording-rules.yml'), 'utf8'),
      fs.readFileSync(path.join(directory, 'alert-rules.yml'), 'utf8'),
    ].join('\n').toLowerCase();
    for (const fragment of FORBIDDEN_RULE_FRAGMENTS) {
      if (ruleText.includes(fragment)) errors.push(`rule files contain forbidden fragment ${fragment}`);
    }
    return errors;
  }

  #checkAlert(errors, rule, runbook) {
    const name = rule.alert ?? '<unnamed>';
    if (!rule.expr) errors.push(`alert ${name} has no expression`);
    if (!rule.for) errors.push(`alert ${name} has no hold duration`);
    const labelKeys = Object.keys(rule.labels ?? {}).sort();
    if (labelKeys.join(',') !== 'severity,subsystem') {
      errors.push(`alert ${name} labels must be exactly severity and subsystem`);
    }
    if (!['warning', 'critical'].includes(rule.labels?.severity)) {
      errors.push(`alert ${name} has unsupported severity`);
    }
    for (const key of ['summary', 'description', 'runbook_url']) {
      if (!rule.annotations?.[key]) errors.push(`alert ${name} is missing annotation ${key}`);
    }
    const anchor = name.toLowerCase();
    const expectedUrl = 'https://github.com/ITmedes/browserpane/blob/main/'
      + `docs/operations/PROMETHEUS_ALERT_RUNBOOK.md#${anchor}`;
    if (rule.annotations?.runbook_url !== expectedUrl) {
      errors.push(`alert ${name} has an invalid runbook URL`);
    }
    if (!runbook.includes(`## ${name}\n`)) {
      errors.push(`alert ${name} has no matching runbook heading`);
    }
  }

  #rules(document) {
    return (document.groups ?? []).flatMap((group) => group.rules ?? []);
  }

  #sameSet(errors, description, actual, expected) {
    const actualValues = new Set(actual ?? []);
    const expectedValues = expected instanceof Set ? expected : new Set(expected);
    const missing = [...expectedValues].filter((value) => !actualValues.has(value));
    const unexpected = [...actualValues].filter((value) => !expectedValues.has(value));
    if (missing.length > 0) errors.push(`${description} missing: ${missing.join(', ')}`);
    if (unexpected.length > 0) errors.push(`${description} unexpected: ${unexpected.join(', ')}`);
  }
}
