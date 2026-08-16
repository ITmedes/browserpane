import { randomBytes } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { DockerCommandRunner } from './docker-command-runner.mjs';
import { PROMETHEUS_IMAGE } from './observability-toolchain.mjs';

export class GrafanaValidationStack {
  #root;
  #runner;
  #suffixFactory;
  #network = null;
  #project = null;
  #fixture = null;
  #temporaryDirectory = null;
  #password = null;

  constructor(root, runner = new DockerCommandRunner(), suffixFactory = defaultSuffix) {
    this.#root = root;
    this.#runner = runner;
    this.#suffixFactory = suffixFactory;
  }

  start() {
    const suffix = this.#suffixFactory();
    this.#network = `bpane-observe-${suffix}`;
    this.#project = `bpane-observe-${suffix}`;
    this.#fixture = `${this.#project}-gateway`;
    this.#password = randomBytes(24).toString('hex');
    this.#temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'bpane-observe-'));
    const fixtureConfig = path.join(this.#temporaryDirectory, 'prometheus.yml');
    fs.writeFileSync(fixtureConfig, 'global:\n  scrape_interval: 1m\nscrape_configs: []\n');

    try {
      this.#runner.run(['network', 'create', '--internal', this.#network]);
      this.#startFixture(fixtureConfig);
      this.#runner.run([
        'compose', '--project-name', this.#project,
        '-f', this.#composePath(), 'up', '-d', '--wait', '--wait-timeout', '120',
      ], { cwd: this.#root, env: this.#environment() });
      const grafana = this.#composeContainer('grafana');
      const prometheus = this.#composeContainer('prometheus');
      this.#assertNoHostPorts(grafana);
      this.#assertNoHostPorts(prometheus);
      return { grafana, prometheus };
    } catch (error) {
      this.stop();
      throw this.#redact(error);
    }
  }

  stop() {
    if (this.#project) {
      this.#runner.tryRun([
        'compose', '--project-name', this.#project,
        '-f', this.#composePath(), 'down', '--remove-orphans', '--volumes',
      ], { cwd: this.#root, env: this.#environment() });
    }
    if (this.#fixture) this.#runner.tryRun(['rm', '--force', this.#fixture]);
    if (this.#network) this.#runner.tryRun(['network', 'rm', this.#network]);
    if (this.#temporaryDirectory) {
      fs.rmSync(this.#temporaryDirectory, { recursive: true, force: true });
    }
    this.#network = null;
    this.#project = null;
    this.#fixture = null;
    this.#temporaryDirectory = null;
    this.#password = null;
  }

  #startFixture(configPath) {
    this.#runner.run([
      'run', '--detach', '--name', this.#fixture,
      '--network', this.#network, '--network-alias', 'gateway',
      '--read-only', '--cap-drop', 'ALL',
      '--security-opt', 'no-new-privileges:true',
      '--tmpfs', '/prometheus:rw,noexec,nosuid,size=32m,uid=65534,gid=65534,mode=0700',
      '--volume', `${configPath}:/etc/prometheus/prometheus.yml:ro`,
      '--entrypoint', '/bin/prometheus', PROMETHEUS_IMAGE,
      '--config.file=/etc/prometheus/prometheus.yml',
      '--storage.tsdb.path=/prometheus', '--web.listen-address=0.0.0.0:8932',
    ]);
  }

  #composeContainer(service) {
    const container = this.#runner.run([
      'compose', '--project-name', this.#project,
      '-f', this.#composePath(), 'ps', '--quiet', service,
    ], { cwd: this.#root, env: this.#environment() });
    if (!container) throw new Error(`Grafana validation ${service} container is missing`);
    return container;
  }

  #assertNoHostPorts(container) {
    if (this.#runner.run(['port', container])) {
      throw new Error('Grafana validation service published a host port');
    }
  }

  #composePath() {
    return path.join(this.#root, 'deploy/examples/observability/compose.grafana.yml');
  }

  #environment() {
    return {
      ...process.env,
      BPANE_GRAFANA_ADMIN_PASSWORD: this.#password ?? 'cleanup-only',
      BPANE_OBSERVABILITY_NETWORK: this.#network ?? 'cleanup-only',
    };
  }

  #redact(error) {
    const message = error instanceof Error ? error.message : String(error);
    return new Error(this.#password ? message.replaceAll(this.#password, '[REDACTED]') : message);
  }
}

function defaultSuffix() {
  return `${process.pid}-${randomBytes(4).toString('hex')}`;
}
