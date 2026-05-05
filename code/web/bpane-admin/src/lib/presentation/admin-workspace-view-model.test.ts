import { describe, expect, it } from 'vitest';
import { AdminWorkspaceViewModelBuilder } from './admin-workspace-view-model';

describe('AdminWorkspaceViewModelBuilder', () => {
  it('keeps the browser stage separate from the test-embed feature groups', () => {
    const viewModel = AdminWorkspaceViewModelBuilder.build({
      browserStatus: 'Connected',
      selectedSessionId: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
      sessionCount: 2,
      fileCount: 1,
      connected: true,
    });

    expect(viewModel.browser.connectionLabel).toBe('connected');
    expect(viewModel.panels.map((panel) => panel.id)).toEqual([
      'sessions',
      'lifecycle',
      'display',
      'files',
      'policy',
      'workflows',
      'recording',
      'metrics',
      'logs',
    ]);
  });

  it('marks shipped admin panels separately from planned feature panels', () => {
    const viewModel = AdminWorkspaceViewModelBuilder.build({
      browserStatus: 'Disconnected',
      selectedSessionId: null,
      sessionCount: 0,
      fileCount: 0,
      connected: false,
    });

    expect(viewModel.panels.filter((panel) => panel.implemented).map((panel) => panel.id)).toEqual([
      'sessions',
      'lifecycle',
      'display',
      'files',
      'policy',
      'workflows',
      'recording',
      'metrics',
      'logs',
    ]);
  });
});
