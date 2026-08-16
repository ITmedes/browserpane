import fs from 'node:fs';
import path from 'node:path';

import {
  DASHBOARD_UID,
  DATASOURCE_UID,
  EXPECTED_PANELS,
  FORBIDDEN_QUERY_FRAGMENTS,
} from './grafana-dashboard-definition.mjs';
import { GrafanaProvisioningContract } from './grafana-provisioning-contract.mjs';

export class GrafanaDashboardContract {
  #root;

  constructor(root) {
    this.#root = root;
  }

  check() {
    const errors = [];
    const directory = path.join(this.#root, 'deploy/examples/observability');
    const dashboardPath = path.join(
      directory, 'grafana/dashboards/browserpane-operations.json',
    );
    const dashboard = JSON.parse(fs.readFileSync(dashboardPath, 'utf8'));
    errors.push(...new GrafanaProvisioningContract(directory).check());
    this.#checkDashboard(errors, dashboard);
    return errors;
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
