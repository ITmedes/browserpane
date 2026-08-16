import {
  DASHBOARD_UID,
  DATASOURCE_UID,
  EXPECTED_PANELS,
} from './grafana-dashboard-definition.mjs';

const API_ORIGIN = 'http://127.0.0.1:3000';
const CURL = 'curl --fail --silent --show-error '
  + '--user "$GF_SECURITY_ADMIN_USER:$GF_SECURITY_ADMIN_PASSWORD"';

export class GrafanaApiProbe {
  #runner;

  constructor(runner) {
    this.#runner = runner;
  }

  run(grafana, prometheus) {
    this.#checkHealth(this.#get(grafana, '/api/health'));
    this.#checkDatasource(this.#get(grafana, `/api/datasources/uid/${DATASOURCE_UID}`));
    this.#checkDatasourceHealth(this.#get(
      grafana, `/api/datasources/uid/${DATASOURCE_UID}/health`,
    ));
    this.#checkDashboard(this.#get(grafana, `/api/dashboards/uid/${DASHBOARD_UID}`));
    const queryResult = this.#post(grafana, '/api/ds/query', this.#queryRequest());
    this.#checkQueries(queryResult);
    this.#checkLogs(grafana, 'Grafana');
    this.#checkLogs(prometheus, 'Prometheus');
    return {
      panels: EXPECTED_PANELS.length,
      queries: EXPECTED_PANELS.filter((panel) => panel.expr).length,
    };
  }

  #get(container, route) {
    return this.#request(container, `${CURL} ${API_ORIGIN}${route}`);
  }

  #post(container, route, body) {
    const command = `${CURL} --header "Content-Type: application/json" `
      + `--request POST --data-binary @- ${API_ORIGIN}${route}`;
    return this.#request(container, command, JSON.stringify(body));
  }

  #request(container, command, input) {
    const output = this.#runner.run(
      ['exec', '--interactive', container, 'sh', '-c', command],
      { input },
    );
    try {
      return JSON.parse(output);
    } catch {
      throw new Error('Grafana API returned invalid JSON');
    }
  }

  #queryRequest() {
    const queries = EXPECTED_PANELS.filter((panel) => panel.expr).map((panel) => ({
      refId: `P${panel.id}`,
      datasource: { type: 'prometheus', uid: DATASOURCE_UID },
      editorMode: 'code',
      expr: panel.expr,
      instant: true,
      intervalMs: 30000,
      maxDataPoints: 100,
      range: false,
    }));
    return { from: 'now-5m', to: 'now', queries };
  }

  #checkHealth(response) {
    if (response.database !== 'ok' || !response.version) {
      throw new Error('Grafana API health is not ready');
    }
  }

  #checkDatasource(response) {
    if (response.uid !== DATASOURCE_UID || response.type !== 'prometheus'
      || response.url !== 'http://prometheus:9090' || response.readOnly !== true) {
      throw new Error('Grafana datasource provisioning drifted');
    }
  }

  #checkDatasourceHealth(response) {
    if (!['OK', 'Success'].includes(response.status)) {
      throw new Error(`Grafana datasource health failed: ${response.message ?? 'unknown'}`);
    }
  }

  #checkDashboard(response) {
    if (response.dashboard?.uid !== DASHBOARD_UID
      || response.dashboard?.panels?.length !== EXPECTED_PANELS.length
      || response.meta?.folderUid !== 'browserpane'
      || response.meta?.provisioned !== true) {
      throw new Error('Grafana dashboard provisioning drifted');
    }
  }

  #checkQueries(response) {
    for (const panel of EXPECTED_PANELS.filter((candidate) => candidate.expr)) {
      const result = response.results?.[`P${panel.id}`];
      if (!result || result.status !== 200 || result.error || !Array.isArray(result.frames)) {
        throw new Error(`Grafana panel ${panel.id} query failed`);
      }
    }
  }

  #checkLogs(container, service) {
    const logs = this.#runner.run(['logs', container], { includeStderr: true });
    if (logs.split('\n').some((line) => /level=error|^error\b/i.test(line.trim()))) {
      throw new Error(`${service} emitted error-level startup logs`);
    }
  }
}
