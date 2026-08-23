const primary = (id, failureClass, reproductionCommand, isolation = 'none') => ({
  id,
  role: 'primary',
  required_when: 'success',
  failure_class: failureClass,
  reproduction_command: reproductionCommand,
  isolation,
});

const finalStages = [
  {
    id: 'diagnostics',
    role: 'diagnostics',
    required_when: 'failure',
    failure_class: 'harness',
    reproduction_command: 'node scripts/collect-compose-diagnostics.mjs',
    isolation: 'none',
  },
  {
    id: 'cleanup',
    role: 'cleanup',
    required_when: 'always',
    failure_class: 'harness',
    reproduction_command: 'scripts/ci/cleanup-compose.sh; docker logout ghcr.io',
    isolation: 'none',
  },
];

const gatewayStages = [
  primary('registry-auth', 'infrastructure', 'docker login ghcr.io'),
  primary('builder-resolution', 'infrastructure',
    'node scripts/ci/ci-rust-builder-resolver.mjs'),
  primary('native-prerequisites', 'infrastructure', 'command -v cc c++ make cmake pkg-config'),
  primary('rust-toolchain', 'infrastructure', 'rustup show active-toolchain'),
  primary('runtime-prerequisites', 'infrastructure', 'docker compose version'),
  primary('worker-image', 'unknown',
    'docker compose -f deploy/compose.yml --profile workflow build workflow-worker'),
  primary('gateway-suite', 'product', 'scripts/run-gateway-compose-e2e.sh --suite <suite>',
    'resources'),
  ...finalStages,
];

const browserStages = [
  primary('registry-auth', 'infrastructure', 'docker login ghcr.io'),
  primary('builder-resolution', 'infrastructure',
    'node scripts/ci/ci-rust-builder-resolver.mjs'),
  primary('runtime-prerequisites', 'infrastructure', 'docker compose version; chromium --version'),
  primary('client-dependencies', 'unknown', 'node scripts/validate.mjs --stage client-install'),
  primary('worker-image', 'unknown',
    'docker compose -f deploy/compose.yml --profile workflow build workflow-worker'),
  primary('compose-runtime', 'product', 'scripts/run-gateway-compose-e2e.sh --suite stack'),
  primary('browser-validation', 'product',
    'node scripts/validate.mjs --stage <browser-compose-stages>', 'resources'),
  ...finalStages,
];

const adminStages = (compatibility) => [
  primary('registry-auth', 'infrastructure', 'docker login ghcr.io'),
  primary('builder-resolution', 'infrastructure',
    'node scripts/ci/ci-rust-builder-resolver.mjs'),
  primary('runtime-prerequisites', 'infrastructure', 'docker compose version; chromium --version'),
  primary('client-dependencies', 'unknown', 'node scripts/validate.mjs --stage client-install'),
  primary('worker-image', 'unknown',
    'docker compose -f deploy/compose.yml --profile workflow build workflow-worker'),
  primary('compose-runtime', 'product', 'scripts/run-gateway-compose-e2e.sh --suite stack'),
  ...(compatibility ? [primary('egress-fixtures', 'harness',
    'scripts/ci/start-compose-egress-fixtures.sh')] : []),
  primary('admin-validation', 'product',
    `node scripts/run-admin-promotion-validation.mjs ${compatibility ? 'compatibility' : 'unified'}`,
    'resources'),
  ...finalStages,
];

const plans = new Map([
  ['gateway-default', gatewayStages],
  ['gateway-docker-pool', gatewayStages],
  ['browser-integrations', browserStages],
  ['admin-unified', adminStages(false)],
  ['admin-compatibility', adminStages(true)],
]);

export const COMPOSE_TEST_PLAN_FILES = Object.freeze([
  '.github/workflows/compose.yml',
  'quality/compose-stage-evidence-v1.schema.json',
  'scripts/compose-evidence.mjs',
  'scripts/compose-harness.mjs',
  'scripts/compose-evidence/compose-artifact-collector.mjs',
  'scripts/compose-evidence/compose-evidence-aggregator.mjs',
  'scripts/compose-evidence/compose-evidence-finalizer.mjs',
  'scripts/compose-evidence/compose-evidence-store.mjs',
  'scripts/compose-evidence/compose-evidence-validator.mjs',
  'scripts/compose-evidence/compose-failure-classifier.mjs',
  'scripts/compose-evidence/compose-identity-collector.mjs',
  'scripts/compose-evidence/compose-junit-writer.mjs',
  'scripts/compose-evidence/compose-lane-plans.mjs',
  'scripts/compose-evidence/compose-stage-runner.mjs',
  'scripts/compose-harness/cleanup-verifier.mjs',
  'scripts/compose-harness/compose-namespace.mjs',
  'scripts/compose-harness/egress-project-names.mjs',
  'scripts/compose-harness/compose-stage-harness.mjs',
  'scripts/compose-harness/harness-recorder.mjs',
  'scripts/compose-harness/namespace-transports.mjs',
  'scripts/compose-harness/preload.mjs',
  'scripts/compose-harness/readiness-waiter.mjs',
  'scripts/compose-harness/resource-registry.mjs',
  'scripts/ci/cleanup-compose.sh',
  'scripts/collect-compose-diagnostics.mjs',
  'scripts/run-admin-promotion-validation.mjs',
  'scripts/run-gateway-compose-e2e.sh',
  'scripts/validate.mjs',
  'scripts/validation/stage-catalog.mjs',
  'scripts/validation/compose-validation-isolation.mjs',
]);

export class ComposeLanePlanCatalog {
  plan(lane) {
    const stages = plans.get(lane);
    if (!stages) throw new Error(`unsupported Compose evidence lane: ${lane}`);
    return {
      schema_version: 1,
      lane,
      stages: stages.map((stage, index) => ({ ...stage, sequence: index + 1 })),
    };
  }

  lanes() {
    return [...plans.keys()];
  }
}
