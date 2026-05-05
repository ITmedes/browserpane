import type { SessionResource } from '../api/control-types';

export type WorkflowOperationsViewModel = {
  readonly title: string;
  readonly status: string;
  readonly note: string;
  readonly selectedSessionLabel: string;
  readonly canRefresh: boolean;
  readonly canRun: boolean;
  readonly canCancel: boolean;
  readonly canReleaseHold: boolean;
};

export class WorkflowOperationsViewModelBuilder {
  static build(input: {
    readonly selectedSession: SessionResource | null;
    readonly apiAvailable: boolean;
  }): WorkflowOperationsViewModel {
    const hasSession = Boolean(input.selectedSession);
    return {
      title: hasSession ? 'Workflow run controls' : 'Select a session for workflow runs',
      status: input.apiAvailable ? 'ready' : 'api pending',
      note: input.apiAvailable
        ? 'Run workflow definitions against the selected BrowserPane session.'
        : 'Panel shell is present; workflow control API wiring is the next integration slice.',
      selectedSessionLabel: input.selectedSession?.id ?? '--',
      canRefresh: input.apiAvailable,
      canRun: input.apiAvailable && hasSession,
      canCancel: input.apiAvailable && hasSession,
      canReleaseHold: input.apiAvailable && hasSession,
    };
  }
}
