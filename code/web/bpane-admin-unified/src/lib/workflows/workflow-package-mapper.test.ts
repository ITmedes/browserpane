import { describe, expect, it } from 'vitest';

import { WorkflowPackageMapper } from './workflow-package-mapper';

describe('WorkflowPackageMapper', () => {
  it('maps the frozen supported package and publication evidence', () => {
    const packageManifest = WorkflowPackageMapper.toManifest({
      package_id: 'support.intake.v1',
      format_version: 'browserpane.workflow-package/v1',
      runtime: {
        language: 'typescript',
        browserpane_api_version: 'v1',
        node_major_version: 22,
        playwright_major_version: 1,
        playwright_minor_version: 59,
      },
      requirements: {
        default_session: { project_id: null },
        allowed_credential_binding_ids: [],
        allowed_extension_ids: [],
        allowed_file_workspace_ids: [],
      },
      execution: {
        timeout_ms: 60_000,
        assertions: ['receipt'],
        safe_cancellation_points: ['before-submit'],
        side_effect_checkpoints: ['after-submit'],
      },
      publication: {
        reviewer: 'reviewer',
        reviewed_at: '2026-08-20T12:00:00Z',
        decision: 'approved',
        fresh_context_replay: true,
        scenarios: [{ kind: 'happy_path', result: 'passed' }],
      },
    });

    expect(packageManifest?.runtime).toMatchObject({
      language: 'typescript',
      node_major_version: 22,
      playwright_minor_version: 59,
    });
    expect(WorkflowPackageMapper.toCompatibility(
      { state: 'supported', warnings: [] },
      'playwright',
      packageManifest,
    )).toEqual({ state: 'supported', warnings: [] });
  });

  it('rejects a runtime tuple the UI does not support', () => {
    expect(() => WorkflowPackageMapper.toManifest({
      package_id: 'unsupported',
      format_version: 'browserpane.workflow-package/v1',
      runtime: {
        language: 'typescript',
        browserpane_api_version: 'v1',
        node_major_version: 20,
        playwright_major_version: 1,
        playwright_minor_version: 59,
      },
      requirements: {},
      execution: {},
      publication: {},
    })).toThrow('Node version must be 22');
  });
});
