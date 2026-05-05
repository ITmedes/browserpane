import { describe, expect, it } from 'vitest';
import { WorkflowOperationsViewModelBuilder } from './workflow-operations-view-model';

describe('WorkflowOperationsViewModelBuilder', () => {
  it('marks workflow controls pending until the API client is wired', () => {
    const viewModel = WorkflowOperationsViewModelBuilder.build({
      selectedSession: null,
      apiAvailable: false,
    });

    expect(viewModel.status).toBe('api pending');
    expect(viewModel.canRun).toBe(false);
    expect(viewModel.note).toContain('next integration slice');
  });
});
