import { describe, expect, it } from 'vitest';

import {
  createNewFileWorkspaceEditDraft,
  hasNewFileWorkspaceEditChanges,
  mergeProjectsWithSelected,
  validateFileWorkspaceEdit,
} from './file-workspace-edit-view-model';

describe('file workspace edit view model', () => {
  it('validates required metadata and detects new draft changes', () => {
    const draft = createNewFileWorkspaceEditDraft();

    expect(hasNewFileWorkspaceEditChanges(draft)).toBe(false);
    expect(validateFileWorkspaceEdit(draft).fieldErrors.name).toEqual(['Name is required.']);
    expect(hasNewFileWorkspaceEditChanges({ ...draft, name: 'Support inputs' })).toBe(true);
  });

  it('builds owner-scoped create requests from trimmed form values', () => {
    const validation = validateFileWorkspaceEdit({
      ...createNewFileWorkspaceEditDraft(),
      name: '  Support inputs  ',
      description: '  CSV fixtures  ',
      labelsText: 'team=support, purpose=demo',
    });

    expect(validation.valid).toBe(true);
    expect(validation.request).toEqual({
      project_id: null,
      name: 'Support inputs',
      description: 'CSV fixtures',
      labels: {
        team: 'support',
        purpose: 'demo',
      },
    });
  });

  it('requires a project for project-scoped workspaces and validates labels', () => {
    const validation = validateFileWorkspaceEdit({
      ...createNewFileWorkspaceEditDraft(),
      name: 'Support inputs',
      projectBinding: 'project',
      projectId: '',
      labelsText: 'broken-label, empty=',
    });

    expect(validation.valid).toBe(false);
    expect(validation.fieldErrors.projectId).toEqual(['Project-scoped workspaces need a project.']);
    expect(validation.fieldErrors.labels).toEqual([
      'Label entry 1 must use key=value syntax.',
      'Label entry 2 must include a non-empty key and value.',
    ]);
  });

  it('keeps a selected archived project visible when it is missing from options', () => {
    expect(mergeProjectsWithSelected([], 'project-archived')).toEqual([
      {
        id: 'project-archived',
        name: 'Missing project project-...',
        state: 'archived',
      },
    ]);
  });
});
