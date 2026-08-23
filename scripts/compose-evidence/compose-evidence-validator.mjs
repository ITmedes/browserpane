const GIT_PATTERN = /^[0-9a-f]{40}$/;
const SHA_PATTERN = /^[0-9a-f]{64}$/;
const DIGEST_PATTERN = /^sha256:[0-9a-f]{64}$/;
const ID_PATTERN = /^[a-z0-9-]{1,48}$/;
const FAILURE_CLASSES = new Set(['product', 'harness', 'infrastructure', 'unknown']);
const OUTCOMES = new Set(['success', 'failure', 'cancelled', 'skipped', 'unknown']);
const SENSITIVE_PATTERN = /(?:bearer|password|private[_ -]?key|secret|token|https?:\/\/)/i;
const ARTIFACT_PATH_PATTERN = /^(?:attachments\/[a-z0-9][a-z0-9._-]{0,80}\.(?:png|webp|zip)|test-results\/ci-diagnostics\/compose\.log)$/;

export class ComposeEvidenceValidator {
  validatePlan(plan) {
    const errors = [];
    if (plan?.schema_version !== 1 || !ID_PATTERN.test(plan?.lane ?? '')) {
      errors.push('plan-header-invalid');
    }
    if (!Array.isArray(plan?.stages) || plan.stages.length < 1 || plan.stages.length > 32) {
      return [...errors, 'plan-stage-count-invalid'];
    }
    const ids = new Set();
    for (const [index, stage] of plan.stages.entries()) {
      if (!ID_PATTERN.test(stage.id ?? '') || ids.has(stage.id)) errors.push('plan-stage-id-invalid');
      ids.add(stage.id);
      if (stage.sequence !== index + 1) errors.push('plan-stage-sequence-invalid');
      if (!['primary', 'diagnostics', 'cleanup'].includes(stage.role)) {
        errors.push('plan-stage-role-invalid');
      }
      if (!['success', 'failure', 'always'].includes(stage.required_when)) {
        errors.push('plan-stage-requirement-invalid');
      }
      if (!FAILURE_CLASSES.has(stage.failure_class)) errors.push('plan-failure-class-invalid');
      if (!this.#safeCommand(stage.reproduction_command)) errors.push('plan-reproduction-invalid');
    }
    if (plan.stages.filter((stage) => stage.role === 'cleanup').length !== 1) {
      errors.push('plan-cleanup-count-invalid');
    }
    return errors;
  }

  validateStage(result, expected) {
    const errors = [];
    if (!result || result.id !== expected.id || result.sequence !== expected.sequence) {
      return ['stage-identity-invalid'];
    }
    for (const field of ['role', 'required_when', 'reproduction_command']) {
      if (result[field] !== expected[field]) errors.push(`stage-${field}-invalid`);
    }
    if (!OUTCOMES.has(result.outcome)) errors.push('stage-outcome-invalid');
    if (result.outcome === 'success' && result.failure_class !== null) {
      errors.push('successful-stage-has-failure-class');
    }
    if (result.outcome !== 'success' && !FAILURE_CLASSES.has(result.failure_class)) {
      errors.push('failed-stage-class-invalid');
    }
    if (!Number.isInteger(result.duration_ms) || result.duration_ms < 0
      || result.duration_ms > 21_600_000) errors.push('stage-duration-invalid');
    if (typeof result.started_at !== 'string' || !Number.isFinite(Date.parse(result.started_at))) {
      errors.push('stage-start-time-invalid');
    }
    if (result.exit_code !== null && (!Number.isInteger(result.exit_code)
      || result.exit_code < 0 || result.exit_code > 255)) errors.push('stage-exit-code-invalid');
    if (result.signal !== null && !/^SIG[A-Z0-9]+$/.test(result.signal)) {
      errors.push('stage-signal-invalid');
    }
    return errors;
  }

  validateSummary(summary) {
    const errors = [];
    if (summary?.schema_version !== 1 || !ID_PATTERN.test(summary?.lane ?? '')) {
      errors.push('summary-header-invalid');
    }
    if (!['success', 'failure', 'cancelled'].includes(summary?.outcome)) {
      errors.push('summary-outcome-invalid');
    }
    if (!Number.isFinite(Date.parse(summary?.generated_at ?? ''))) {
      errors.push('summary-generated-at-invalid');
    }
    if (!/^\d{1,24}$/.test(summary?.run?.id ?? '')
      || !Number.isInteger(summary?.run?.attempt) || summary.run.attempt < 1
      || summary.run.attempt > 100 || summary.run.first_pass !== (summary.run.attempt === 1)
      || !/^[a-z_]{1,32}$/.test(summary?.run?.event ?? '')) {
      errors.push('summary-run-invalid');
    }
    if (!GIT_PATTERN.test(summary?.identity?.tested_commit ?? '')
      || !GIT_PATTERN.test(summary?.identity?.tested_tree ?? '')) {
      errors.push('summary-git-identity-invalid');
    }
    if (!SHA_PATTERN.test(summary?.identity?.workflow_sha256 ?? '')
      || !SHA_PATTERN.test(summary?.identity?.test_plan_sha256 ?? '')) {
      errors.push('summary-plan-identity-invalid');
    }
    const digests = summary?.identity?.image_digests;
    if (!Array.isArray(digests) || digests.length > 64
      || digests.some((digest) => !DIGEST_PATTERN.test(digest))) {
      errors.push('summary-image-identity-invalid');
    }
    if (!Array.isArray(summary?.stages) || summary.stages.length < 1
      || summary.stages.length > 32) errors.push('summary-stages-invalid');
    if (!Array.isArray(summary?.artifacts) || summary.artifacts.length > 12) {
      errors.push('summary-artifacts-invalid');
    } else {
      const attachmentBytes = summary.artifacts
        .filter((artifact) => artifact.kind !== 'diagnostics')
        .reduce((total, artifact) => total + (artifact.bytes ?? 0), 0);
      if (attachmentBytes > 20 * 1024 * 1024 || summary.artifacts.some((artifact) =>
        !['diagnostics', 'trace', 'screenshot'].includes(artifact.kind)
        || !ARTIFACT_PATH_PATTERN.test(artifact.path ?? '')
        || !Number.isInteger(artifact.bytes) || artifact.bytes < 0
        || artifact.bytes > 5 * 1024 * 1024
        || !SHA_PATTERN.test(artifact.sha256 ?? ''))) {
        errors.push('summary-artifact-entry-invalid');
      }
    }
    if (!Array.isArray(summary?.evidence_errors) || summary.evidence_errors.length > 32) {
      errors.push('summary-errors-invalid');
    }
    if (summary?.primary_failure?.class === 'unknown' && summary.outcome === 'success') {
      errors.push('unknown-failure-passed');
    }
    if (Buffer.byteLength(JSON.stringify(summary)) > 262_144) errors.push('summary-size-invalid');
    return errors;
  }

  #safeCommand(command) {
    return typeof command === 'string' && command.length > 0 && command.length <= 240
      && !SENSITIVE_PATTERN.test(command);
  }
}
