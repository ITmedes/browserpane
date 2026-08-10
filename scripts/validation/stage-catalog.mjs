import path from 'node:path';

import { ADMIN_PROMOTION_SMOKES } from './admin-promotion-contract.mjs';
import { ValidationStage } from './validation-stage.mjs';

export class ValidationStageCatalog {
  #fastStages;
  #composeStages;

  constructor(rootDirectory) {
    this.#fastStages = this.#buildFastStages(rootDirectory);
    this.#composeStages = this.#buildComposeStages(rootDirectory);
  }

  select(profile, requestedIds = []) {
    const available = profile === 'all'
      ? [...this.#fastStages, ...this.#composeStages]
      : this.forProfile(profile);
    if (requestedIds.length === 0) return available;

    const byId = new Map(available.map((stage) => [stage.id, stage]));
    return requestedIds.map((id) => {
      const stage = byId.get(id);
      if (!stage) throw new Error(`unknown validation stage for ${profile}: ${id}`);
      return stage;
    });
  }

  forProfile(profile) {
    if (profile === 'fast') return [...this.#fastStages];
    if (profile === 'compose') return [...this.#composeStages];
    if (profile === 'full') return [...this.#fastStages, ...this.#composeStages];
    throw new Error(`unknown validation profile: ${profile}`);
  }

  #buildFastStages(root) {
    const stages = [
      this.#node('validation-tool-tests', 'Test validation tooling', root, [
        '--test',
        'scripts/dependency-safety/audit-parser.test.mjs',
        'scripts/dependency-safety/audit-runner.test.mjs',
        'scripts/dependency-safety/policy.test.mjs',
        'scripts/coverage/coverage-baseline-checker.test.mjs',
        'scripts/coverage/rust-coverage-command.test.mjs',
        'scripts/ci/ci-rust-builder-consumer-contract.test.mjs',
        'scripts/ci/ci-rust-builder-ref.test.mjs',
        'scripts/ci/ci-rust-builder-resolver.test.mjs',
        'scripts/ci/ci-rust-builder-workflow-contract.test.mjs',
        'scripts/ci/admin-security-headers-contract.test.mjs',
        'scripts/ci/compose-workflow-contract.test.mjs',
        'scripts/ci/compose-diagnostics-collector.test.mjs',
        'scripts/ci/diagnostic-redactor.test.mjs',
        'scripts/ci/gateway-compose-e2e-wrapper.test.mjs',
        'scripts/validation/github-workflow-policy-checker.test.mjs',
        'scripts/validation/markdown-link-checker.test.mjs',
        'scripts/validation/repository-document-checker.test.mjs',
        'code/web/bpane-client/scripts/cdp-profile-state-probe.test.mjs',
        'code/web/bpane-client/scripts/session-cleanup.test.mjs',
        'scripts/validation/arguments.test.mjs',
        'scripts/validation/repository-baseline.test.mjs',
        'scripts/validation/stage-catalog.test.mjs',
        'scripts/validation/subprocess-executor.test.mjs',
        'scripts/validation/validation-runner.test.mjs',
        'scripts/validation/yaml-document-parser.test.mjs'
      ], 180),
      this.#node('repository-baseline', 'Validate canonical paths and manifests', root,
        ['scripts/check-repository-baseline.mjs'], 120),
      this.#node('repository-documents', 'Validate Markdown, YAML, and workflows', root,
        ['scripts/check-repository-documents.mjs'], 180),
      this.#node('dependency-safety', 'Audit all committed dependency locks', root,
        ['scripts/check-dependency-safety.mjs'], 600),
      this.#stage('rust-fmt', 'Check Rust formatting', 'cargo',
        ['fmt', '--all', '--', '--check'], root, 300),
      this.#stage('rust-clippy', 'Run Rust clippy', 'cargo',
        ['clippy', '--workspace', '--all-targets', '--locked'], root, 1200),
      this.#stage('rust-tests', 'Run Rust workspace tests', 'cargo',
        ['test', '--workspace', '--locked'], root, 1200),
      this.#node('rust-coverage', 'Run Rust coverage ratchet', root,
        ['scripts/run-rust-coverage.mjs'], 1800)
    ];
    stages.push(...this.#npmPackage(root, 'admin-auth', 'code/web/bpane-admin-auth',
      ['check', 'test', 'test:coverage', 'build']));
    stages.push(...this.#npmPackage(root, 'admin', 'code/web/bpane-admin',
      ['check', 'test', 'build']));
    stages.push(...this.#npmPackage(root, 'admin-new', 'code/web/bpane-admin-unified',
      ['check', 'test', 'test:coverage', 'build']));
    stages.push(...this.#npmPackage(root, 'client', 'code/web/bpane-client',
      ['check', 'test', 'test:coverage', 'build']));
    stages.push(...this.#npmPackage(root, 'mcp-bridge', 'code/integrations/mcp-bridge',
      ['test', 'build']));
    stages.push(...this.#npmPackage(root, 'recording-worker',
      'code/integrations/recording-worker', ['build']));
    stages.push(...this.#npmPackage(root, 'workflow-worker',
      'code/integrations/workflow-worker', ['build']));
    stages.push(...this.#npmPackage(root, 'openapi', 'scripts/openapi',
      ['test', 'check', 'compatibility']));
    stages.push(this.#node('egress-observer-check', 'Parse egress observer example', root,
      ['--check', 'deploy/examples/egress-observer/egress-usage-reporter.mjs'], 120));
    stages.push(this.#node('egress-observer-tests', 'Test egress observer example', root,
      ['--test', 'deploy/examples/egress-observer/egress-usage-reporter.test.mjs'], 180));
    return stages;
  }

  #buildComposeStages(root) {
    const client = path.join(root, 'code/web/bpane-client');
    return [
      this.#stage('compose-gateway-api', 'Run gateway compose API suites', 'bash',
        ['scripts/run-gateway-compose-e2e.sh', '--suite', 'all'], root, 2700),
      ...ADMIN_PROMOTION_SMOKES.map(({ id, description, script, extraArgs }) => (
        this.#npmSmoke(id, description, client, script, extraArgs)
      )),
      this.#npmSmoke('compose-cli', 'Smoke BrowserPane CLI', client, 'smoke:bpane-cli'),
      this.#npmSmoke('compose-session-files', 'Smoke session file evidence', client,
        'smoke:session-files'),
      this.#npmSmoke('compose-mcp', 'Smoke session-scoped MCP endpoint', client,
        'smoke:mcp-session-endpoints', ['--connect-timeout-ms', '60000']),
      this.#npmSmoke('compose-recording', 'Smoke recording lifecycle', client, 'smoke:recording'),
      this.#npmSmoke('compose-workflow', 'Smoke workflow admission', client,
        'smoke:workflow-admission'),
      this.#npmSmoke('compose-workflow-cli', 'Smoke workflow CLI compatibility', client,
        'smoke:workflow-cli'),
      this.#npmSmoke('compose-workflow-workspace', 'Smoke workflow workspace transfer', client,
        'smoke:workflow-workspace'),
      this.#npmSmoke('compose-workflow-events', 'Smoke workflow event delivery', client,
        'smoke:workflow-events')
    ];
  }

  #npmPackage(root, id, relativeDirectory, scripts) {
    const cwd = path.join(root, relativeDirectory);
    const stages = [this.#stage(`${id}-install`, `Clean install ${id}`, 'npm',
      ['ci', '--ignore-scripts'], cwd, 600)];
    for (const script of scripts) {
      stages.push(this.#stage(`${id}-${script.replace(':', '-')}`, `Run ${id} ${script}`,
        'npm', ['run', script], cwd, 900));
    }
    return stages;
  }

  #npmSmoke(id, description, cwd, script, extraArgs = []) {
    return this.#stage(id, description, 'npm',
      ['run', script, '--', '--headless', ...extraArgs], cwd, 900);
  }

  #node(id, description, cwd, args, timeoutSeconds) {
    return this.#stage(id, description, process.execPath, args, cwd, timeoutSeconds);
  }

  #stage(id, description, command, args, cwd, timeoutSeconds) {
    return new ValidationStage({ id, description, command, args, cwd, timeoutSeconds });
  }
}
