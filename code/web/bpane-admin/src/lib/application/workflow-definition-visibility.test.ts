import { describe, expect, it } from 'vitest';
import type { WorkflowDefinitionResource } from '../api/workflow-types';
import {
  hiddenWorkflowDefinitions,
  visibleWorkflowDefinitions,
  workflowDefinitionKind,
} from './workflow-definition-visibility';

describe('workflow definition visibility', () => {
  it('keeps example templates visible and hides smoke definitions by default', () => {
    const visible = visibleWorkflowDefinitions([
      SMOKE_WORKFLOW,
      USER_WORKFLOW,
      TOUR_WORKFLOW,
      DRAFT_TEMPLATE,
    ]);

    expect(visible.map((definition) => definition.id)).toEqual([
      TOUR_WORKFLOW.id,
      USER_WORKFLOW.id,
    ]);
    expect(hiddenWorkflowDefinitions([SMOKE_WORKFLOW, TOUR_WORKFLOW, DRAFT_TEMPLATE]).map((definition) => definition.id))
      .toEqual([SMOKE_WORKFLOW.id, DRAFT_TEMPLATE.id]);
    expect(workflowDefinitionKind(TOUR_WORKFLOW)).toBe('Example template');
    expect(workflowDefinitionKind(USER_WORKFLOW)).toBe('Workflow');
  });
});

const BASE_WORKFLOW = {
  description: null,
  labels: {},
  latest_version: 'v1',
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:00:00Z',
} satisfies Omit<WorkflowDefinitionResource, 'id' | 'name'>;

const SMOKE_WORKFLOW: WorkflowDefinitionResource = {
  ...BASE_WORKFLOW,
  id: 'workflow-smoke',
  name: 'admin-workflow-smoke-1778576705740',
  labels: { suite: 'admin-workflow-smoke' },
};

const TOUR_WORKFLOW: WorkflowDefinitionResource = {
  ...BASE_WORKFLOW,
  id: 'workflow-tour',
  name: 'BrowserPane Tour',
  labels: { bpane_admin_template: 'browserpane-tour', source: 'bpane-admin-template' },
};

const DRAFT_TEMPLATE: WorkflowDefinitionResource = {
  ...BASE_WORKFLOW,
  id: 'workflow-draft-template',
  name: 'Draft Template',
  labels: { bpane_admin_template: 'draft-template' },
  latest_version: null,
};

const USER_WORKFLOW: WorkflowDefinitionResource = {
  ...BASE_WORKFLOW,
  id: 'workflow-user',
  name: 'Customer Onboarding',
  labels: { source: 'operator' },
};
