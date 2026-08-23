export class ComposeEvidenceAggregator {
  #validator;
  #clock;

  constructor(validator, clock = () => new Date()) {
    this.#validator = validator;
    this.#clock = clock;
  }

  aggregate(plan, rawResults, identityResult, artifactResult, environment) {
    const errors = [...this.#validator.validatePlan(plan)];
    const results = this.#validatedResults(plan, rawResults, errors);
    const stages = this.#materializeStages(plan, results, errors);
    const primaryStage = stages.find((stage) => stage.role === 'primary'
      && ['failure', 'cancelled', 'unknown'].includes(stage.outcome));
    const cleanupStage = stages.find((stage) => stage.role === 'cleanup');
    errors.push(...identityResult.errors, ...artifactResult.errors);
    if (stages.some((stage) => stage.id === 'worker-image' && stage.outcome === 'success')
      && identityResult.identity.image_digests.length === 0) {
      errors.push('identity-image-digests-empty');
    }
    const boundedErrors = [...new Set(errors)].slice(0, 32);
    const cleanup = {
      outcome: cleanupStage?.outcome ?? 'unknown',
      duration_ms: cleanupStage?.duration_ms ?? null,
      exit_code: cleanupStage?.exit_code ?? null,
      signal: cleanupStage?.signal ?? null,
    };
    const summary = {
      schema_version: 1,
      generated_at: this.#clock().toISOString(),
      run: this.#runIdentity(environment, boundedErrors),
      identity: identityResult.identity,
      lane: plan.lane,
      outcome: this.#outcome(primaryStage, cleanup, boundedErrors),
      primary_failure: this.#primaryFailure(primaryStage),
      cleanup,
      stages,
      artifacts: artifactResult.artifacts,
      evidence_errors: boundedErrors,
    };
    const validationErrors = this.#validator.validateSummary(summary);
    if (validationErrors.length > 0) {
      summary.evidence_errors = [...new Set([...summary.evidence_errors, ...validationErrors])]
        .slice(0, 32);
      summary.outcome = 'failure';
      if (!summary.primary_failure && summary.cleanup.outcome === 'success') {
        summary.primary_failure = this.#finalizationFailure();
      }
    }
    if (summary.evidence_errors.length > 0 && summary.outcome === 'success') {
      summary.outcome = 'failure';
      summary.primary_failure = this.#finalizationFailure();
    }
    return summary;
  }

  #validatedResults(plan, rawResults, errors) {
    const results = new Map();
    for (const result of rawResults) {
      const expected = plan.stages.find((stage) => stage.id === result?.id);
      if (!expected) {
        errors.push('unexpected-stage-result');
        continue;
      }
      if (results.has(result.id)) {
        errors.push('duplicate-stage-result');
        continue;
      }
      const validationErrors = this.#validator.validateStage(result, expected);
      if (validationErrors.length > 0) {
        errors.push(...validationErrors);
        continue;
      }
      results.set(result.id, result);
    }
    return results;
  }

  #materializeStages(plan, results, errors) {
    let primarySequence = plan.stages
      .filter((stage) => stage.role === 'primary')
      .map((stage) => results.get(stage.id))
      .filter((stage) => stage && stage.outcome !== 'success')
      .map((stage) => stage.sequence)
      .sort((left, right) => left - right)[0] ?? null;
    return plan.stages.map((stage) => {
      const result = results.get(stage.id);
      if (result) return result;
      if (stage.role === 'primary' && primarySequence !== null
        && stage.sequence > primarySequence) return this.#synthetic(stage, 'skipped', null);
      if (stage.role === 'diagnostics' && primarySequence === null) {
        return this.#synthetic(stage, 'skipped', null);
      }
      errors.push(`required-stage-missing-${stage.id}`);
      if (stage.role === 'primary' && primarySequence === null) primarySequence = stage.sequence;
      return this.#synthetic(stage, 'unknown', 'unknown');
    });
  }

  #synthetic(stage, outcome, failureClass) {
    return {
      ...stage,
      outcome,
      failure_class: failureClass,
      started_at: null,
      duration_ms: null,
      exit_code: null,
      signal: null,
    };
  }

  #runIdentity(environment, errors) {
    const attempt = Number.parseInt(environment.GITHUB_RUN_ATTEMPT ?? '1', 10);
    const validId = /^\d{1,24}$/.test(environment.GITHUB_RUN_ID ?? '');
    const validEvent = /^[a-z_]{1,32}$/.test(environment.GITHUB_EVENT_NAME ?? '');
    const id = validId ? environment.GITHUB_RUN_ID : '0';
    const event = validEvent ? environment.GITHUB_EVENT_NAME : 'local';
    if (!validId) errors.push('run-id-invalid');
    if (!validEvent) errors.push('run-event-invalid');
    if (!Number.isInteger(attempt) || attempt < 1 || attempt > 100) {
      errors.push('run-attempt-invalid');
    }
    const safeAttempt = Number.isInteger(attempt) && attempt >= 1 && attempt <= 100 ? attempt : 1;
    return { id, attempt: safeAttempt, event, first_pass: safeAttempt === 1 };
  }

  #outcome(primaryStage, cleanup, errors) {
    if (primaryStage?.outcome === 'cancelled') return 'cancelled';
    if (primaryStage || cleanup.outcome !== 'success' || errors.length > 0) return 'failure';
    return 'success';
  }

  #primaryFailure(stage) {
    if (!stage) return null;
    return {
      stage_id: stage.id,
      class: stage.failure_class ?? 'unknown',
      outcome: stage.outcome === 'cancelled' ? 'cancelled' : 'failure',
      exit_code: stage.exit_code,
      signal: stage.signal,
    };
  }

  #finalizationFailure() {
    return {
      stage_id: 'evidence-finalization',
      class: 'unknown',
      outcome: 'failure',
      exit_code: 1,
      signal: null,
    };
  }
}
