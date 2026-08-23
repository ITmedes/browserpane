const BOUNDARIES = new Set([
  'oidc',
  'control',
  'runtime',
  'transport',
  'workflow_worker',
  'recording_worker',
  'artifact',
]);

export class ComposeHarnessError extends Error {
  constructor(code, message, details = {}, options = {}) {
    super(message, options);
    this.name = 'ComposeHarnessError';
    this.code = code;
    this.details = Object.freeze({ ...details });
  }
}

export class ReadinessWaiter {
  #observe;
  #clock;
  #wait;
  #record;

  constructor({ observe, clock = Date.now, wait = delay, record = () => {} }) {
    if (typeof observe !== 'function' || typeof clock !== 'function'
      || typeof wait !== 'function' || typeof record !== 'function') {
      throw new TypeError('ReadinessWaiter requires observe, clock, wait, and record functions.');
    }
    this.#observe = observe;
    this.#clock = clock;
    this.#wait = wait;
    this.#record = record;
  }

  async waitFor(boundary, options = {}) {
    validateBoundary(boundary);
    const timeoutMs = positiveInteger(options.timeoutMs ?? 60_000, 'timeoutMs');
    const intervalMs = nonNegativeInteger(options.intervalMs ?? 500, 'intervalMs');
    const startedAt = this.#clock();
    const deadline = startedAt + timeoutMs;
    let attempts = 0;
    let lastState = null;
    let dependencyLosses = 0;

    while (this.#clock() <= deadline) {
      if (options.signal?.aborted) {
        throw this.#failure('cancelled', boundary, attempts, startedAt, deadline, lastState);
      }
      attempts += 1;
      try {
        const observed = await this.#observe(boundary, options.signal);
        lastState = summarizeState(observed);
        if (isStaleResource(observed, options.expectedResourceId)) {
          throw this.#failure('stale_resource', boundary, attempts, startedAt, deadline, lastState);
        }
        const predicate = options.isReady ?? boundaryReady;
        if (predicate(boundary, observed)) {
          const result = this.#result(boundary, attempts, startedAt, deadline, lastState);
          this.#record({ type: 'readiness', outcome: 'ready', ...result });
          return { ...result, value: observed };
        }
      } catch (error) {
        if (error instanceof ComposeHarnessError) throw error;
        dependencyLosses += 1;
        lastState = { status: 'dependency_unavailable' };
      }

      if (this.#clock() >= deadline) break;
      await this.#wait(Math.min(intervalMs, Math.max(0, deadline - this.#clock())), options.signal);
    }

    const code = dependencyLosses > 0 ? 'dependency_loss' : 'readiness_timeout';
    throw this.#failure(code, boundary, attempts, startedAt, deadline, lastState);
  }

  #failure(code, boundary, attempts, startedAt, deadline, lastState) {
    const result = this.#result(boundary, attempts, startedAt, deadline, lastState);
    this.#record({ type: 'readiness', outcome: code, ...result });
    return new ComposeHarnessError(
      code,
      `Compose ${boundary} readiness failed: ${code}.`,
      result,
    );
  }

  #result(boundary, attempts, startedAt, deadline, lastState) {
    return {
      boundary,
      attempts,
      elapsed_ms: Math.max(0, this.#clock() - startedAt),
      deadline_ms: Math.max(0, deadline - startedAt),
      last_state: lastState,
    };
  }
}

export function boundaryReady(boundary, state) {
  validateBoundary(boundary);
  if (!state || typeof state !== 'object') return false;
  if (boundary === 'oidc') {
    return typeof state.issuer === 'string' && typeof state.token_endpoint === 'string';
  }
  if (boundary === 'control') {
    return state.status === 'ready' && state.lifecycle === 'running'
      && dependencyReady(state, 'session_store');
  }
  if (boundary === 'runtime') {
    return state.status === 'ready' && dependencyReady(state, 'runtime_manager');
  }
  if (boundary === 'transport') {
    return state.connected === true && state.application_ready === true;
  }
  if (boundary === 'workflow_worker') {
    return ['running', 'succeeded', 'failed', 'cancelled', 'awaiting_input'].includes(state.state);
  }
  if (boundary === 'recording_worker') {
    return ['recording', 'finalizing', 'completed', 'failed'].includes(state.state);
  }
  return state.available === true || typeof state.artifact_ref === 'string';
}

export function summarizeState(value) {
  if (!value || typeof value !== 'object') return value ?? null;
  const summary = {};
  for (const key of [
    'status', 'state', 'lifecycle', 'connected', 'application_ready', 'available', 'resource_id',
    'active_sessions', 'owned_containers', 'owned_volumes', 'retained_volumes',
    'unexpected_containers', 'unexpected_volumes',
  ]) {
    const candidate = value[key];
    if (['string', 'boolean', 'number'].includes(typeof candidate)) summary[key] = candidate;
  }
  if (Array.isArray(value.checks)) {
    summary.checks = value.checks.slice(0, 16).map((check) => ({
      name: typeof check?.name === 'string' ? check.name : 'unknown',
      status: typeof check?.status === 'string' ? check.status : 'unknown',
    }));
  }
  return summary;
}

function dependencyReady(state, name) {
  return Array.isArray(state.checks)
    && state.checks.some((check) => check?.name === name && check?.status === 'ready');
}

function isStaleResource(state, expectedResourceId) {
  return typeof expectedResourceId === 'string' && expectedResourceId.length > 0
    && typeof state?.resource_id === 'string' && state.resource_id !== expectedResourceId;
}

function validateBoundary(boundary) {
  if (!BOUNDARIES.has(boundary)) throw new TypeError(`Unsupported readiness boundary: ${boundary}`);
}

function positiveInteger(value, name) {
  if (!Number.isInteger(value) || value <= 0) throw new TypeError(`${name} must be a positive integer.`);
  return value;
}

function nonNegativeInteger(value, name) {
  if (!Number.isInteger(value) || value < 0) throw new TypeError(`${name} must be a non-negative integer.`);
  return value;
}

function delay(durationMs, signal) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(resolve, durationMs);
    signal?.addEventListener('abort', () => {
      clearTimeout(timer);
      reject(new ComposeHarnessError('cancelled', 'Compose readiness wait cancelled.'));
    }, { once: true });
  });
}
