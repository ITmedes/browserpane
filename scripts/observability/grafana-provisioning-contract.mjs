import path from 'node:path';

import { YamlDocumentParser } from '../validation/yaml-document-parser.mjs';
import {
  DATASOURCE_UID,
  GRAFANA_IMAGE,
} from './grafana-dashboard-definition.mjs';

export class GrafanaProvisioningContract {
  #directory;
  #parser;

  constructor(directory, parser = new YamlDocumentParser()) {
    this.#directory = directory;
    this.#parser = parser;
  }

  check() {
    const errors = [];
    this.#checkDatasource(errors, this.#parser.parse(path.join(
      this.#directory, 'grafana/provisioning/datasources/prometheus.yml',
    )));
    this.#checkProvider(errors, this.#parser.parse(path.join(
      this.#directory, 'grafana/provisioning/dashboards/browserpane.yml',
    )));
    this.#checkCompose(errors, this.#parser.parse(path.join(
      this.#directory, 'compose.grafana.yml',
    )));
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
      uid: DATASOURCE_UID, type: 'prometheus', access: 'proxy',
      isDefault: true, editable: false,
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
    if (!String(prometheus.image ?? '').includes('@sha256:')) errors.push('Prometheus image must be immutable');
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
}
