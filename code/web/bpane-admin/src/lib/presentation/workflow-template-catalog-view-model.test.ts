import { describe, expect, it } from 'vitest';
import type {
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
  WorkflowRunResource,
} from '../api/workflow-types';
import { WorkflowTemplateCatalogViewModelBuilder } from './workflow-template-catalog-view-model';

describe('WorkflowTemplateCatalogViewModelBuilder', () => {
  it('summarizes visible workflow templates for the catalog', () => {
    const viewModel = WorkflowTemplateCatalogViewModelBuilder.catalog({
      items: [{ definition: TOUR_WORKFLOW, latestVersion: TOUR_VERSION, versionError: null }],
      hiddenCount: 2,
      search: 'github',
    });

    expect(viewModel.rows).toHaveLength(1);
    expect(viewModel.rows[0]?.kind).toBe('Example template');
    expect(viewModel.rows[0]?.executor).toBe('playwright');
    expect(viewModel.rows[0]?.source).toContain('/workspace');
    expect(viewModel.rows[0]?.sourceCommit).toBe('abc123');
    expect(viewModel.hiddenCount).toBe(2);
  });

  it('builds version details and recent run links', () => {
    const viewModel = WorkflowTemplateCatalogViewModelBuilder.detail({
      definition: TOUR_WORKFLOW,
      versions: [OLD_VERSION, TOUR_VERSION],
      selectedVersion: '',
      runs: [OTHER_RUN, TOUR_RUN],
    });

    expect(viewModel.latestVersion).toBe('v1');
    expect(viewModel.versionRows.map((row) => [row.version, row.latest])).toEqual([
      ['v0', false],
      ['v1', true],
    ]);
    expect(viewModel.selectedVersion?.version).toBe('v1');
    expect(viewModel.selectedVersion?.sourceRows.find((row) => row.label === 'Resolved commit')?.value)
      .toBe('abc123');
    expect(viewModel.recentRuns.map((run) => run.id)).toEqual([TOUR_RUN.id]);
  });
});

const TOUR_WORKFLOW: WorkflowDefinitionResource = {
  id: 'workflow-tour',
  name: 'BrowserPane Tour',
  description: 'Example workflow that tours browserpane.io and GitHub',
  labels: { bpane_admin_template: 'browserpane-tour', source: 'bpane-admin-template' },
  latest_version: 'v1',
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:03:00Z',
};

const TOUR_VERSION: WorkflowDefinitionVersionResource = {
  id: 'version-tour',
  workflow_definition_id: TOUR_WORKFLOW.id,
  version: 'v1',
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
  default_session: null,
  allowed_credential_binding_ids: [],
  allowed_extension_ids: [],
  allowed_file_workspace_ids: [],
  created_at: '2026-05-04T19:02:00Z',
};

const OLD_VERSION: WorkflowDefinitionVersionResource = {
  ...TOUR_VERSION,
  id: 'version-old',
  version: 'v0',
  created_at: '2026-05-04T19:01:00Z',
};

const TOUR_RUN: WorkflowRunResource = {
  id: 'run-tour',
  workflow_definition_id: TOUR_WORKFLOW.id,
  workflow_definition_version_id: TOUR_VERSION.id,
  workflow_version: 'v1',
  state: 'succeeded',
  session_id: 'session-tour',
  automation_task_id: 'task-tour',
  input: {},
  output: {},
  error: null,
  artifact_refs: [],
  produced_files: [],
  intervention: { pending_request: null },
  runtime: null,
  labels: {},
  events_path: '/api/v1/workflow-runs/run-tour/events',
  logs_path: '/api/v1/workflow-runs/run-tour/logs',
  created_at: '2026-05-04T19:04:00Z',
  updated_at: '2026-05-04T19:05:00Z',
};

const OTHER_RUN: WorkflowRunResource = {
  ...TOUR_RUN,
  id: 'run-other',
  workflow_definition_id: 'workflow-other',
};
