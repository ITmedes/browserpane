import { describe, expect, it, vi } from 'vitest';
import type { WorkflowClient } from '../api/workflow-client';
import type {
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
} from '../api/workflow-types';
import { WorkflowOperationsService } from './workflow-operations-service';

describe('WorkflowOperationsService', () => {
  it('registers the BrowserPane Tour template and hides smoke definitions', async () => {
    const client = {
      listDefinitions: vi.fn().mockResolvedValue({ workflows: [SMOKE_WORKFLOW] }),
      createDefinition: vi.fn().mockResolvedValue(TOUR_WORKFLOW_DRAFT),
      createDefinitionVersion: vi.fn().mockResolvedValue(TOUR_VERSION),
      getDefinition: vi.fn().mockResolvedValue(TOUR_WORKFLOW),
      getDefinitionVersion: vi.fn().mockResolvedValue(TOUR_VERSION),
    } as unknown as WorkflowClient;
    const service = new WorkflowOperationsService(client);

    const selection = await service.loadDefinitions('', '');

    expect(selection.definitions).toEqual([TOUR_WORKFLOW]);
    expect(selection.selectedWorkflowId).toBe(TOUR_WORKFLOW.id);
    expect(selection.selectedVersion).toBe('v1');
    expect(selection.selectedVersionResource?.executor).toBe('playwright');
    expect(client.createDefinition).toHaveBeenCalledWith(expect.objectContaining({
      name: 'BrowserPane Tour',
      labels: expect.objectContaining({ bpane_admin_template: 'browserpane-tour' }),
    }));
    expect(client.createDefinitionVersion).toHaveBeenCalledWith(
      TOUR_WORKFLOW_DRAFT.id,
      expect.objectContaining({
        executor: 'playwright',
        entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
        source: expect.objectContaining({ repository_url: '/workspace', root_path: 'dev' }),
      }),
    );
  });

  it('keeps an existing template visible while preserving an explicit user selection', async () => {
    const client = {
      listDefinitions: vi.fn().mockResolvedValue({
        workflows: [SMOKE_WORKFLOW, USER_WORKFLOW, TOUR_WORKFLOW],
      }),
      createDefinition: vi.fn(),
      createDefinitionVersion: vi.fn(),
      getDefinition: vi.fn(),
      getDefinitionVersion: vi.fn().mockResolvedValue(USER_VERSION),
    } as unknown as WorkflowClient;
    const service = new WorkflowOperationsService(client);

    const selection = await service.loadDefinitions(USER_WORKFLOW.id, 'v1');

    expect(selection.definitions.map((definition) => definition.id)).toEqual([
      TOUR_WORKFLOW.id,
      USER_WORKFLOW.id,
    ]);
    expect(selection.selectedWorkflowId).toBe(USER_WORKFLOW.id);
    expect(selection.selectedVersionResource).toEqual(USER_VERSION);
    expect(client.createDefinition).not.toHaveBeenCalled();
    expect(client.createDefinitionVersion).not.toHaveBeenCalled();
  });
});

const SMOKE_WORKFLOW: WorkflowDefinitionResource = {
  id: 'workflow-smoke',
  name: 'admin-workflow-smoke-1778576705740',
  description: 'Admin smoke workflow',
  labels: { suite: 'admin-workflow-smoke' },
  latest_version: 'v1',
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:00:00Z',
};

const TOUR_WORKFLOW_DRAFT: WorkflowDefinitionResource = {
  id: 'workflow-tour',
  name: 'BrowserPane Tour',
  description: 'Example workflow that tours browserpane.io and the BrowserPane GitHub repository',
  labels: { bpane_admin_template: 'browserpane-tour', source: 'bpane-admin-template' },
  latest_version: null,
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:00:00Z',
};

const TOUR_WORKFLOW: WorkflowDefinitionResource = {
  ...TOUR_WORKFLOW_DRAFT,
  latest_version: 'v1',
};

const USER_WORKFLOW: WorkflowDefinitionResource = {
  id: 'workflow-user',
  name: 'Customer Onboarding',
  description: null,
  labels: { source: 'operator' },
  latest_version: 'v1',
  created_at: '2026-05-04T19:01:00Z',
  updated_at: '2026-05-04T19:01:00Z',
};

const TOUR_VERSION: WorkflowDefinitionVersionResource = {
  id: 'version-tour',
  workflow_definition_id: TOUR_WORKFLOW.id,
  version: 'v1',
  executor: 'playwright',
  entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
  source: { kind: 'git', repository_url: '/workspace', ref: 'HEAD', root_path: 'dev' },
  input_schema: { type: 'object' },
  output_schema: null,
  default_session: null,
  allowed_credential_binding_ids: [],
  allowed_extension_ids: [],
  allowed_file_workspace_ids: [],
  created_at: '2026-05-04T19:00:00Z',
};

const USER_VERSION: WorkflowDefinitionVersionResource = {
  id: 'version-user',
  workflow_definition_id: USER_WORKFLOW.id,
  version: 'v1',
  executor: 'playwright',
  entrypoint: 'workflows/customer/run.mjs',
  input_schema: null,
  output_schema: null,
  default_session: null,
  allowed_credential_binding_ids: [],
  allowed_extension_ids: [],
  allowed_file_workspace_ids: [],
  created_at: '2026-05-04T19:01:00Z',
};
