import fs from 'node:fs';
import path from 'node:path';

import { YamlDocumentParser } from '../validation/yaml-document-parser.mjs';

export const GRAFANA_IMAGE = 'grafana/grafana-oss@sha256:'
  + '5dad0df181cb644a14e13617b913b261a54f7d4fd4510721dba420929f35bea2';
export const DASHBOARD_UID = 'browserpane-operations';
export const DATASOURCE_UID = 'browserpane-prometheus';

const EXPECTED_PANELS = [
  { id: 1, title: 'Scope and interpretation', type: 'text' },
  { id: 2, title: 'Gateway scrape health', type: 'stat', unit: 'short', expr: 'max(up{job="browserpane-gateway"})' },
  { id: 3, title: 'Gateway request rate', type: 'timeseries', unit: 'reqps', expr: 'browserpane:gateway_http_requests:rate5m' },
  { id: 4, title: 'Gateway server-error ratio', type: 'timeseries', unit: 'percentunit', expr: 'browserpane:gateway_http_5xx:ratio_rate5m' },
  { id: 5, title: 'Gateway p95 response duration', type: 'timeseries', unit: 's', expr: 'browserpane:gateway_http_request_duration_seconds:p95_rate5m' },
  { id: 6, title: 'Runtime assignment utilization', type: 'gauge', unit: 'percentunit', expr: 'browserpane:runtime_capacity_utilization:ratio' },
  { id: 7, title: 'Active runtime assignments', type: 'stat', unit: 'short', expr: 'browserpane_gateway_runtime_active_assignments' },
  { id: 8, title: 'Starting runtime assignments', type: 'stat', unit: 'short', expr: 'browserpane_gateway_runtime_starting_assignments' },
  { id: 9, title: 'Runtime assignment limit', type: 'stat', unit: 'short', expr: 'browserpane_gateway_runtime_assignment_limit' },
  { id: 10, title: 'Produced-file upload failure ratio', type: 'stat', unit: 'percentunit', expr: 'browserpane:workflow_produced_file_upload_failure:ratio_increase15m' },
  { id: 11, title: 'Produced-file upload failures', type: 'stat', unit: 'short', expr: 'browserpane:workflow_produced_file_upload_failures:increase15m' },
  { id: 12, title: 'Event-delivery success ratio', type: 'stat', unit: 'percentunit', expr: 'browserpane:workflow_event_delivery_success:ratio_increase15m' },
  { id: 13, title: 'Event-delivery retries', type: 'stat', unit: 'short', expr: 'browserpane:workflow_event_delivery_retries:increase15m' },
  { id: 14, title: 'Event-delivery terminal failures', type: 'stat', unit: 'short', expr: 'browserpane:workflow_event_delivery_failures:increase15m' },
  { id: 15, title: 'Recording finalization success ratio', type: 'stat', unit: 'percentunit', expr: 'browserpane:recording_artifact_finalize_success:ratio_increase15m' },
  { id: 16, title: 'Recording finalization failures', type: 'stat', unit: 'short', expr: 'browserpane:recording_artifact_finalize_failures:increase15m' },
  { id: 17, title: 'Recording worker failures', type: 'stat', unit: 'short', expr: 'browserpane:recording_failures:increase15m' },
  { id: 18, title: 'Playback export failures', type: 'stat', unit: 'short', expr: 'browserpane:recording_playback_export_failures:increase15m' },
  { id: 19, title: 'Workflow retention failures', type: 'stat', unit: 'short', expr: 'browserpane:workflow_retention_failures:increase1h' },
  { id: 20, title: 'Recording retention failures', type: 'stat', unit: 'short', expr: 'browserpane:recording_retention_failures:increase1h' },
];

const FORBIDDEN_QUERY_FRAGMENTS = [
  'owner_id', 'project_id', 'session_id', 'workflow_id', 'recording_id',
  'target_url', 'authorization', 'bearer', 'credential', 'payload',
  'artifact_ref', 'raw_error', 'browser_content', '{{', '$',
];

export class GrafanaDashboardContract {
  #root;
  #parser;

  constructor(root, parser = new YamlDocumentParser()) {
    this.#root = root;
    this.#parser = parser;
  }

  check() {
    const errors = [];
    const directory = path.join(this.#root, 'deploy/examples/observability');
    const dashboardPath = path.join(
      directory, 'grafana/dashboards/browserpane-operations.json',
    );
    const dashboard = JSON.parse(fs.readFileSync(dashboardPath, 'utf8'));
    const datasource = this.#parser.parse(path.join(
      directory, 'grafana/provisioning/datasources/prometheus.yml',
    ));
    const provider = this.#parser.parse(path.join(
      directory, 'grafana/provisioning/dashboards/browserpane.yml',
    ));
    const compose = this.#parser.parse(path.join(directory, 'compose.grafana.yml'));

    this.#checkDatasource(errors, datasource);
    this.#checkProvider(errors, provider);
    this.#checkCompose(errors, compose);
    this.#checkDashboard(errors, dashboard);
    return errors;
  }

  #checkDatasource(errors, document) {
    if (document.apiVersion !== 1 || document.prune !== true) {
      errors.push('Grafana datasource provisioning must use apiVersion 1 with prune');
    }
    const datasources = document.datasources ?? [];
    if (datasources.length !== 1) errors.push('exactly one Grafana datasource is required');
    const datasource = datasources[0] ?? {};
    const expected = {
      uid: DATASOURCE_UID,
      type: 'prometheus',
      access: 'proxy',
      isDefault: true,
      editable: false,
    };
    for (const [key, value] of Object.entries(expected)) {
      if (datasource[key] !== value) errors.push(`Grafana datasource ${key} must be ${value}`);
    }
    if (datasource.secureJsonData || datasource.basicAuth || datasource.user) {
      errors.push('Grafana datasource must not contain embedded credentials');
    }
    try {
      const url = new URL(datasource.url);
      if (url.protocol !== 'http:' || url.hostname !== 'prometheus' || url.port !== '9090'
        || url.username || url.password || url.search || url.hash || url.pathname !== '/') {
        errors.push('Grafana datasource URL must be the credential-free private Prometheus root');
      }
    } catch {
      errors.push('Grafana datasource URL must be valid');
    }
  }

  #checkProvider(errors, document) {
    if (document.apiVersion !== 1) errors.push('Grafana dashboard provider must use apiVersion 1');
    const providers = document.providers ?? [];
    if (providers.length !== 1) errors.push('exactly one Grafana dashboard provider is required');
    const provider = providers[0] ?? {};
    if (provider.type !== 'file' || provider.folderUid !== 'browserpane') {
      errors.push('Grafana dashboard provider must target the stable BrowserPane folder');
    }
    if (provider.allowUiUpdates !== false || provider.disableDeletion !== true) {
      errors.push('Grafana dashboard provisioning must remain repository-authoritative');
    }
    if (provider.options?.path !== '/var/lib/grafana/dashboards'
      || provider.options?.foldersFromFilesStructure !== false) {
      errors.push('Grafana dashboard provider path is invalid');
    }
  }

  #checkCompose(errors, document) {
    const prometheus = document.services?.prometheus ?? {};
    const grafana = document.services?.grafana ?? {};
    if (grafana.image !== GRAFANA_IMAGE) errors.push('Grafana image is not the approved immutable image');
    if (!String(prometheus.image ?? '').includes('@sha256:')) {
      errors.push('Prometheus image must be immutable');
    }
    for (const [name, service] of Object.entries({ prometheus, grafana })) {
      if (service.ports) errors.push(`${name} must not publish host ports`);
      if (service.read_only !== true) errors.push(`${name} root filesystem must be read-only`);
      if (!(service.cap_drop ?? []).includes('ALL')) errors.push(`${name} must drop all capabilities`);
      if (!(service.security_opt ?? []).includes('no-new-privileges:true')) {
        errors.push(`${name} must set no-new-privileges`);
      }
    }
    const environment = grafana.environment ?? {};
    if (environment.GF_AUTH_ANONYMOUS_ENABLED !== 'false'
      || environment.GF_USERS_ALLOW_SIGN_UP !== 'false') {
      errors.push('Grafana anonymous access and sign-up must be disabled');
    }
    if (environment.GF_PLUGINS_PREINSTALL_DISABLED !== 'true'
      || environment.GF_PLUGINS_PREINSTALL_AUTO_UPDATE !== 'false') {
      errors.push('Grafana plugin discovery and automatic updates must be disabled');
    }
    if (!String(environment.GF_SECURITY_ADMIN_PASSWORD ?? '').includes(':?')) {
      errors.push('Grafana administrator password must be operator-supplied');
    }
    const network = document.networks?.default ?? {};
    if (network.external !== true || !String(network.name ?? '').includes(':?')) {
      errors.push('observability services must join an explicit external private network');
    }
  }

  #checkDashboard(errors, dashboard) {
    const expectedRoot = {
      uid: DASHBOARD_UID,
      title: 'BrowserPane Operations',
      editable: false,
      refresh: '30s',
      timezone: 'browser',
      graphTooltip: 1,
    };
    for (const [key, value] of Object.entries(expectedRoot)) {
      if (dashboard[key] !== value) errors.push(`Grafana dashboard ${key} must be ${value}`);
    }
    if (!Number.isInteger(dashboard.schemaVersion) || dashboard.schemaVersion < 41) {
      errors.push('Grafana dashboard schemaVersion must be at least 41');
    }
    if (dashboard.time?.from !== 'now-6h' || dashboard.time?.to !== 'now') {
      errors.push('Grafana dashboard must use the stable six-hour default range');
    }
    if ((dashboard.templating?.list ?? []).length !== 0) {
      errors.push('Grafana dashboard must not define resource variables');
    }
    if ((dashboard.annotations?.list ?? []).length !== 0) {
      errors.push('Grafana dashboard must not define dynamic annotations');
    }
    const links = dashboard.links ?? [];
    const expectedLink = 'https://github.com/ITmedes/browserpane/blob/main/'
      + 'docs/operations/PROMETHEUS_ALERT_RUNBOOK.md';
    if (links.length !== 1 || links[0]?.url !== expectedLink || links[0]?.includeVars !== false) {
      errors.push('Grafana dashboard must contain only the static alert-runbook link');
    }

    const panels = dashboard.panels ?? [];
    if (panels.length !== EXPECTED_PANELS.length) {
      errors.push(`Grafana dashboard must contain ${EXPECTED_PANELS.length} panels`);
    }
    const ids = panels.map((panel) => panel.id);
    if (new Set(ids).size !== ids.length
      || !EXPECTED_PANELS.every((panel) => ids.includes(panel.id))) {
      errors.push('Grafana panel ids must be unique and match the stable inventory');
    }
    this.#checkGrid(errors, panels);

    for (const expected of EXPECTED_PANELS) {
      const panel = panels.find((candidate) => candidate.id === expected.id);
      if (!panel) continue;
      if (panel.title !== expected.title || panel.type !== expected.type) {
        errors.push(`Grafana panel ${expected.id} title or type drifted`);
      }
      if (String(panel.description ?? '').length < 40) {
        errors.push(`Grafana panel ${expected.id} needs an operator description`);
      }
      if (expected.type === 'text') {
        if (panel.datasource !== null || panel.targets) {
          errors.push('Grafana interpretation panel must not query a datasource');
        }
        continue;
      }
      if (panel.datasource?.uid !== DATASOURCE_UID) {
        errors.push(`Grafana panel ${expected.id} uses an unexpected datasource`);
      }
      if (panel.fieldConfig?.defaults?.unit !== expected.unit) {
        errors.push(`Grafana panel ${expected.id} has the wrong unit`);
      }
      if (!panel.fieldConfig?.defaults?.noValue) {
        errors.push(`Grafana panel ${expected.id} must describe no-data behavior`);
      }
      const targets = panel.targets ?? [];
      if (targets.length !== 1 || targets[0]?.refId !== 'A'
        || targets[0]?.datasource?.uid !== DATASOURCE_UID
        || targets[0]?.expr !== expected.expr) {
        errors.push(`Grafana panel ${expected.id} query contract drifted`);
      }
      const queryMetadata = `${targets[0]?.expr ?? ''}\n${targets[0]?.legendFormat ?? ''}`.toLowerCase();
      for (const fragment of FORBIDDEN_QUERY_FRAGMENTS) {
        if (queryMetadata.includes(fragment)) {
          errors.push(`Grafana panel ${expected.id} contains forbidden query fragment ${fragment}`);
        }
      }
    }
  }

  #checkGrid(errors, panels) {
    for (const panel of panels) {
      const grid = panel.gridPos ?? {};
      if (![grid.x, grid.y, grid.w, grid.h].every(Number.isInteger)
        || grid.x < 0 || grid.y < 0 || grid.w <= 0 || grid.h <= 0
        || grid.x + grid.w > 24) {
        errors.push(`Grafana panel ${panel.id} has invalid grid geometry`);
      }
    }
    for (let left = 0; left < panels.length; left += 1) {
      for (let right = left + 1; right < panels.length; right += 1) {
        if (this.#overlaps(panels[left].gridPos, panels[right].gridPos)) {
          errors.push(`Grafana panels ${panels[left].id} and ${panels[right].id} overlap`);
        }
      }
    }
  }

  #overlaps(left, right) {
    return left.x < right.x + right.w && left.x + left.w > right.x
      && left.y < right.y + right.h && left.y + left.h > right.y;
  }
}
