import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { GrafanaDashboardContract } from './grafana-dashboard-contract.mjs';

const ROOT = path.resolve(import.meta.dirname, '../..');

test('repository Grafana dashboard satisfies the operations contract', () => {
  assert.deepEqual(new GrafanaDashboardContract(ROOT).check(), []);
});

test('contract rejects resource variables, unsafe queries, and layout drift', (context) => {
  const fixture = copyFixture(context);
  const dashboardPath = path.join(
    fixture,
    'deploy/examples/observability/grafana/dashboards/browserpane-operations.json',
  );
  const dashboard = JSON.parse(fs.readFileSync(dashboardPath, 'utf8'));
  dashboard.templating.list.push({ name: 'session_id', query: 'label_values(session_id)' });
  dashboard.panels[1].targets[0].expr = 'browserpane_metric{owner_id="$owner_id"}';
  dashboard.panels[2].id = dashboard.panels[1].id;
  dashboard.panels[3].gridPos = { ...dashboard.panels[2].gridPos };
  fs.writeFileSync(dashboardPath, `${JSON.stringify(dashboard, null, 2)}\n`);

  const errors = new GrafanaDashboardContract(fixture).check();
  assert(errors.some((error) => error.includes('must not define resource variables')));
  assert(errors.some((error) => error.includes('panel ids must be unique')));
  assert(errors.some((error) => error.includes('query contract drifted')));
  assert(errors.some((error) => error.includes('forbidden query fragment owner_id')));
  assert(errors.some((error) => error.includes('overlap')));
});

test('contract rejects credentials, public ports, and mutable provisioning', (context) => {
  const fixture = copyFixture(context);
  replace(
    fixture,
    'deploy/examples/observability/grafana/provisioning/datasources/prometheus.yml',
    'url: http://prometheus:9090',
    'url: http://user:secret@prometheus:9090?unsafe=true',
  );
  replace(
    fixture,
    'deploy/examples/observability/grafana/provisioning/dashboards/browserpane.yml',
    'allowUiUpdates: false',
    'allowUiUpdates: true',
  );
  replace(
    fixture,
    'deploy/examples/observability/compose.grafana.yml',
    '    expose:\n      - "3000"',
    '    ports:\n      - "3000:3000"',
  );

  const errors = new GrafanaDashboardContract(fixture).check();
  assert(errors.some((error) => error.includes('credential-free private Prometheus root')));
  assert(errors.some((error) => error.includes('repository-authoritative')));
  assert(errors.some((error) => error.includes('must not publish host ports')));
});

test('contract rejects misleading no-activity colors and cramped panels', (context) => {
  const fixture = copyFixture(context);
  const dashboardPath = path.join(
    fixture,
    'deploy/examples/observability/grafana/dashboards/browserpane-operations.json',
  );
  const dashboard = JSON.parse(fs.readFileSync(dashboardPath, 'utf8'));
  dashboard.panels[0].gridPos.h = 3;
  dashboard.panels[11].gridPos.w = 4;
  dashboard.panels[11].fieldConfig.defaults.mappings = [];
  fs.writeFileSync(dashboardPath, `${JSON.stringify(dashboard, null, 2)}\n`);

  const errors = new GrafanaDashboardContract(fixture).check();
  assert(errors.some((error) => error.includes('interpretation guidance')));
  assert(errors.some((error) => error.includes('too narrow for operator labels')));
  assert(errors.some((error) => error.includes('map no activity to a neutral color')));
});

function copyFixture(context) {
  const fixture = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-grafana-dashboard-'));
  context.after(() => fs.rmSync(fixture, { recursive: true, force: true }));
  fs.mkdirSync(path.join(fixture, 'deploy/examples'), { recursive: true });
  fs.cpSync(
    path.join(ROOT, 'deploy/examples/observability'),
    path.join(fixture, 'deploy/examples/observability'),
    { recursive: true },
  );
  return fixture;
}

function replace(root, relativePath, search, replacement) {
  const target = path.join(root, relativePath);
  const source = fs.readFileSync(target, 'utf8');
  const updated = source.replace(search, replacement);
  assert.notEqual(updated, source, `fixture replacement did not match ${relativePath}`);
  fs.writeFileSync(target, updated);
}
