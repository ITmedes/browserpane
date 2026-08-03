const SHA_PATTERN = /^[0-9a-f]{40}$/;
const ALLOWED_ARTIFACT_ROOTS = [
  'test-results/coverage',
  'test-results/ci-diagnostics'
];

export class GitHubWorkflowPolicyChecker {
  check(relativePath, workflow) {
    const errors = [];
    if (!workflow || typeof workflow !== 'object') {
      return [`${relativePath} must contain a YAML object`];
    }
    this.#checkTrigger(relativePath, workflow, errors);
    this.#checkPermissions(relativePath, workflow.permissions, errors);
    if (!workflow.jobs || typeof workflow.jobs !== 'object') {
      errors.push(`${relativePath} must define jobs`);
      return errors;
    }
    for (const [jobId, job] of Object.entries(workflow.jobs)) {
      this.#checkJob(relativePath, jobId, job, errors);
    }
    if (JSON.stringify(workflow).includes('secrets.')) {
      errors.push(`${relativePath} must not reference repository or environment secrets`);
    }
    return errors;
  }

  #checkTrigger(path, workflow, errors) {
    const trigger = workflow.on;
    if (trigger && typeof trigger === 'object' && 'pull_request_target' in trigger) {
      errors.push(`${path} must not use pull_request_target`);
    }
  }

  #checkPermissions(path, permissions, errors) {
    const keys = permissions && typeof permissions === 'object' ? Object.keys(permissions) : [];
    if (keys.length !== 1 || permissions.contents !== 'read') {
      errors.push(`${path} root permissions must be exactly contents: read`);
    }
  }

  #checkJob(path, jobId, job, errors) {
    const prefix = `${path} job ${jobId}`;
    if (job['runs-on'] !== 'ubuntu-24.04') {
      errors.push(`${prefix} must pin runs-on to ubuntu-24.04`);
    }
    const timeout = job['timeout-minutes'];
    if (!Number.isInteger(timeout) || timeout < 1 || timeout > 60) {
      errors.push(`${prefix} must set timeout-minutes between 1 and 60`);
    }
    if (job.permissions !== undefined) {
      this.#checkPermissions(prefix, job.permissions, errors);
    }
    for (const step of job.steps ?? []) this.#checkStep(prefix, step, errors);
  }

  #checkStep(prefix, step, errors) {
    if (!step.uses) return;
    if (step.uses.startsWith('./')) return;
    const separator = step.uses.lastIndexOf('@');
    const revision = separator >= 0 ? step.uses.slice(separator + 1) : '';
    if (!SHA_PATTERN.test(revision)) {
      errors.push(`${prefix} must pin action to a full commit SHA: ${step.uses}`);
    }
    const action = separator >= 0 ? step.uses.slice(0, separator) : step.uses;
    if (action === 'actions/checkout' && step.with?.['persist-credentials'] !== false) {
      errors.push(`${prefix} checkout must set persist-credentials: false`);
    }
    if (action === 'actions/setup-node' && step.with?.cache
      && !step.with['cache-dependency-path']) {
      errors.push(`${prefix} setup-node caching requires cache-dependency-path`);
    }
    if (action === 'actions/cache' && !String(step.with?.key ?? '').includes('hashFiles(')) {
      errors.push(`${prefix} cache key must include hashFiles(...)`);
    }
    if (action === 'actions/upload-artifact') this.#checkArtifact(prefix, step, errors);
  }

  #checkArtifact(prefix, step, errors) {
    const retention = Number(step.with?.['retention-days']);
    if (!Number.isInteger(retention) || retention < 1 || retention > 14) {
      errors.push(`${prefix} artifact retention must be between 1 and 14 days`);
    }
    const paths = String(step.with?.path ?? '').split('\n').map((value) => value.trim()).filter(Boolean);
    if (paths.length === 0 || paths.some((value) => !ALLOWED_ARTIFACT_ROOTS.some(
      (root) => value === root || value.startsWith(`${root}/`)
    ))) {
      errors.push(`${prefix} artifact paths must use an approved test-results root`);
    }
  }
}
