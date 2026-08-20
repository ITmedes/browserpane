import type {
  WorkflowPackageCompatibility,
  WorkflowPackageManifest,
  WorkflowPackageScenarioKind,
} from './workflow-types';

const FORMAT = 'browserpane.workflow-package/v1' as const;
const SCENARIOS = new Set<WorkflowPackageScenarioKind>([
  'happy_path',
  'validation',
  'missing_element',
  'authentication_challenge',
  'portal_failure',
  'runtime_failure',
  'cancellation',
  'ambiguous_post_side_effect',
]);

export class WorkflowPackageMapper {
  static toManifest(value: unknown): WorkflowPackageManifest | null {
    if (value === null || value === undefined) {
      return null;
    }
    const packageObject = this.#record(value, 'workflow package');
    const runtime = this.#record(packageObject.runtime, 'workflow package runtime');
    const requirements = this.#record(packageObject.requirements, 'workflow package requirements');
    const execution = this.#record(packageObject.execution, 'workflow package execution');
    const publication = this.#record(packageObject.publication, 'workflow package publication');
    this.#literal(packageObject.format_version, FORMAT, 'workflow package format_version');
    this.#literal(runtime.language, 'typescript', 'workflow package runtime language');
    this.#literal(runtime.browserpane_api_version, 'v1', 'workflow package runtime API version');
    this.#literal(runtime.node_major_version, 22, 'workflow package runtime Node version');
    this.#literal(runtime.playwright_major_version, 1, 'workflow package runtime Playwright major');
    this.#literal(runtime.playwright_minor_version, 59, 'workflow package runtime Playwright minor');
    this.#literal(publication.decision, 'approved', 'workflow package publication decision');
    this.#literal(publication.fresh_context_replay, true, 'workflow package fresh-context replay');
    return {
      package_id: this.#string(packageObject.package_id, 'workflow package id'),
      format_version: FORMAT,
      runtime: {
        language: 'typescript',
        browserpane_api_version: 'v1',
        node_major_version: 22,
        playwright_major_version: 1,
        playwright_minor_version: 59,
      },
      requirements: {
        default_session: requirements.default_session,
        allowed_credential_binding_ids: this.#strings(
          requirements.allowed_credential_binding_ids,
          'workflow package credential bindings',
        ),
        allowed_extension_ids: this.#strings(
          requirements.allowed_extension_ids,
          'workflow package extensions',
        ),
        allowed_file_workspace_ids: this.#strings(
          requirements.allowed_file_workspace_ids,
          'workflow package file workspaces',
        ),
      },
      execution: {
        timeout_ms: this.#number(execution.timeout_ms, 'workflow package timeout'),
        assertions: this.#strings(execution.assertions, 'workflow package assertions'),
        safe_cancellation_points: this.#strings(
          execution.safe_cancellation_points,
          'workflow package cancellation points',
        ),
        side_effect_checkpoints: this.#strings(
          execution.side_effect_checkpoints,
          'workflow package side-effect checkpoints',
        ),
      },
      publication: {
        reviewer: this.#string(publication.reviewer, 'workflow package reviewer'),
        reviewed_at: this.#string(publication.reviewed_at, 'workflow package reviewed_at'),
        decision: 'approved',
        fresh_context_replay: true,
        scenarios: this.#array(publication.scenarios, 'workflow package scenarios').map((value) => {
          const scenario = this.#record(value, 'workflow package scenario');
          const kind = this.#string(scenario.kind, 'workflow package scenario kind');
          if (!SCENARIOS.has(kind as WorkflowPackageScenarioKind)) {
            throw new TypeError(`workflow package scenario kind ${kind} is not supported`);
          }
          const result = this.#string(scenario.result, 'workflow package scenario result');
          if (result !== 'passed' && result !== 'not_applicable') {
            throw new TypeError(`workflow package scenario result ${result} is not supported`);
          }
          return { kind: kind as WorkflowPackageScenarioKind, result };
        }),
      },
    };
  }

  static toCompatibility(
    value: unknown,
    executor: string,
    packageManifest: WorkflowPackageManifest | null,
  ): WorkflowPackageCompatibility {
    if (value === undefined) {
      if (executor !== 'playwright') {
        return { state: 'unsupported', warnings: ['Unsupported legacy executor.'] };
      }
      return packageManifest
        ? { state: 'supported', warnings: [] }
        : { state: 'legacy', warnings: ['This version predates the Phase 0 package manifest.'] };
    }
    const compatibility = this.#record(value, 'workflow package compatibility');
    const state = this.#string(compatibility.state, 'workflow package compatibility state');
    if (state !== 'supported' && state !== 'legacy' && state !== 'unsupported') {
      throw new TypeError(`workflow package compatibility state ${state} is not supported`);
    }
    return {
      state,
      warnings: this.#strings(compatibility.warnings, 'workflow package compatibility warnings'),
    };
  }

  static #record(value: unknown, label: string): Record<string, unknown> {
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
      throw new TypeError(`${label} must be an object`);
    }
    return value as Record<string, unknown>;
  }

  static #array(value: unknown, label: string): unknown[] {
    if (!Array.isArray(value)) {
      throw new TypeError(`${label} must be an array`);
    }
    return value;
  }

  static #string(value: unknown, label: string): string {
    if (typeof value !== 'string') {
      throw new TypeError(`${label} must be a string`);
    }
    return value;
  }

  static #strings(value: unknown, label: string): string[] {
    return this.#array(value, label).map((item) => this.#string(item, label));
  }

  static #number(value: unknown, label: string): number {
    if (typeof value !== 'number' || !Number.isFinite(value)) {
      throw new TypeError(`${label} must be a finite number`);
    }
    return value;
  }

  static #literal<T extends string | number | boolean>(value: unknown, expected: T, label: string): void {
    if (value !== expected) {
      throw new TypeError(`${label} must be ${String(expected)}`);
    }
  }
}
