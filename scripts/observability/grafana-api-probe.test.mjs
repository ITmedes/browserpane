import assert from 'node:assert/strict';
import test from 'node:test';

import { EXPECTED_PANELS } from './grafana-dashboard-definition.mjs';
import { GrafanaApiProbe } from './grafana-api-probe.mjs';

test('Grafana API probe verifies provisioned resources and every panel query', () => {
  const runner = new FakeGrafanaRunner();

  const evidence = new GrafanaApiProbe(runner).run('grafana-id', 'prometheus-id');

  assert.deepEqual(evidence, { panels: 20, queries: 19 });
  assert.equal(runner.queryRefs.size, 19);
  assert(EXPECTED_PANELS.filter((panel) => panel.expr)
    .every((panel) => runner.queryRefs.has(`P${panel.id}`)));
});

test('Grafana API probe rejects datasource, dashboard, and query failures', () => {
  const datasource = new FakeGrafanaRunner({ datasourceUrl: 'http://unsafe:9090' });
  assert.throws(
    () => new GrafanaApiProbe(datasource).run('grafana-id', 'prometheus-id'),
    /datasource provisioning drifted/,
  );

  const dashboard = new FakeGrafanaRunner({ panelCount: 1 });
  assert.throws(
    () => new GrafanaApiProbe(dashboard).run('grafana-id', 'prometheus-id'),
    /dashboard provisioning drifted/,
  );

  const query = new FakeGrafanaRunner({ failedQuery: 'P4' });
  assert.throws(
    () => new GrafanaApiProbe(query).run('grafana-id', 'prometheus-id'),
    /panel 4 query failed/,
  );
});

test('Grafana API probe rejects error-level service logs', () => {
  const runner = new FakeGrafanaRunner({ logs: 'level=error msg="unsafe startup"' });
  assert.throws(
    () => new GrafanaApiProbe(runner).run('grafana-id', 'prometheus-id'),
    /Grafana emitted error-level startup logs/,
  );
});

class FakeGrafanaRunner {
  queryRefs = new Set();
  #options;

  constructor(options = {}) {
    this.#options = options;
  }

  run(args, options = {}) {
    if (args[0] === 'logs') return this.#options.logs ?? '';
    const command = args.at(-1);
    if (command.includes('/api/ds/query')) return this.#queries(options.input);
    if (command.includes('/api/dashboards/uid/')) return this.#dashboard();
    if (command.includes('/health') && command.includes('/datasources/')) {
      return JSON.stringify({ status: 'OK', message: 'success' });
    }
    if (command.includes('/api/datasources/uid/')) return this.#datasource();
    if (command.includes('/api/health')) return JSON.stringify({ database: 'ok', version: '13.0.2' });
    throw new Error(`unexpected fake Docker command: ${args.join(' ')}`);
  }

  #datasource() {
    return JSON.stringify({
      uid: 'browserpane-prometheus', type: 'prometheus',
      url: this.#options.datasourceUrl ?? 'http://prometheus:9090', readOnly: true,
    });
  }

  #dashboard() {
    return JSON.stringify({
      dashboard: {
        uid: 'browserpane-operations',
        panels: Array.from({ length: this.#options.panelCount ?? 20 }),
      },
      meta: { folderUid: 'browserpane', provisioned: true },
    });
  }

  #queries(input) {
    const request = JSON.parse(input);
    this.queryRefs = new Set(request.queries.map((query) => query.refId));
    const results = Object.fromEntries(request.queries.map((query) => [query.refId, {
      status: query.refId === this.#options.failedQuery ? 500 : 200,
      frames: [],
    }]));
    return JSON.stringify({ results });
  }
}
