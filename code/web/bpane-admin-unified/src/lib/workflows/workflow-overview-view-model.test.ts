import { describe, expect, it } from 'vitest';

import {
  buildWorkflowDefinitionDetailModel,
  buildWorkflowOverviewModel,
  labelSummary,
  workflowMatchesSearch,
} from './workflow-overview-view-model';
import type {
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
} from './workflow-types';
import {
  ADMIN_TEMPLATE_LABEL,
  BROWSERPANE_TOUR_TEMPLATE,
} from './workflow-visibility';

describe('workflow overview view model', () => {
  it('summarizes catalog metrics and latest-version metadata', () => {
    const model = buildWorkflowOverviewModel([
      workflow({ id: 'tour', template: true }),
      workflow({ id: 'plain', latest: null }),
    ], {
      tour: version(),
      plain: null,
    }, 2);

    expect(model.metrics.map((metric) => [metric.label, metric.value])).toEqual([
      ['Visible definitions', '2'],
      ['Templates', '1'],
      ['Published', '1'],
      ['Hidden internal', '2'],
    ]);
    expect(model.rows[0]).toMatchObject({
      id: 'tour',
      kind: 'Example template',
      latestVersion: 'v1',
      executor: 'playwright',
      source: 'git /workspace:dev',
      sourceCommit: 'abc123',
    });
    expect(model.rows[1]).toMatchObject({
      id: 'plain',
      latestVersion: 'No version',
      executor: 'No published version',
    });
  });

  it('matches catalog rows by source, executor, label, and version', () => {
    const [row] = buildWorkflowOverviewModel([workflow({ id: 'tour', template: true })], {
      tour: version(),
    }).rows;

    expect(workflowMatchesSearch(row!, 'playwright')).toBe(true);
    expect(workflowMatchesSearch(row!, '/workspace')).toBe(true);
    expect(workflowMatchesSearch(row!, 'v1')).toBe(true);
    expect(workflowMatchesSearch(row!, 'browserpane-tour')).toBe(true);
    expect(workflowMatchesSearch(row!, 'missing')).toBe(false);
  });

  it('builds detail metadata for selected versions', () => {
    const model = buildWorkflowDefinitionDetailModel({
      definition: workflow({ id: 'tour', template: true }),
      versions: [version(), version({ id: 'version-2', version: 'v2' })],
      selectedVersion: 'v2',
    });

    expect(model).toMatchObject({
      definitionId: 'tour',
      kind: 'Example template',
      latestVersion: 'v1',
      selectedVersion: {
        id: 'version-2',
        version: 'v2',
        executor: 'playwright',
      },
    });
    expect(model.selectedVersion?.policyRows).toContainEqual({
      label: 'File workspaces',
      value: 'workspace-1',
    });
    expect(labelSummary({ z: 'last', a: 'first' })).toBe('a=first, z=last');
  });
});

function workflow(options: {
  readonly id: string;
  readonly latest?: string | null;
  readonly template?: boolean;
}): WorkflowDefinitionResource {
  return {
    id: options.id,
    name: options.template ? 'BrowserPane Tour' : 'Customer workflow',
    description: null,
    labels: options.template ? { [ADMIN_TEMPLATE_LABEL]: BROWSERPANE_TOUR_TEMPLATE } : { team: 'support' },
    latest_version: options.latest === undefined ? 'v1' : options.latest,
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}

function version(overrides: Partial<{ readonly id: string; readonly version: string }> = {}): WorkflowDefinitionVersionResource {
  return {
    id: overrides.id ?? 'version-1',
    workflow_definition_id: 'tour',
    version: overrides.version ?? 'v1',
    executor: 'playwright',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
      root_path: 'dev',
    },
    input_schema: { type: 'object' },
    output_schema: null,
    default_session: { labels: { purpose: 'tour' } },
    allowed_credential_binding_ids: [],
    allowed_extension_ids: ['extension-1'],
    allowed_file_workspace_ids: ['workspace-1'],
    created_at: '2026-06-21T09:30:00.000Z',
  };
}
