import type { AdminWorkflowRunSnapshot } from '../api/admin-event-snapshots';
import type { ControlClient } from '../api/control-client';
import type { SessionResource } from '../api/control-types';

export type AdminWorkflowSessionFollowerOptions = {
  readonly controlClient: Pick<ControlClient, 'getSession'>;
  readonly getSessions: () => readonly SessionResource[];
  readonly getConnectedSessionId: () => string | null;
  readonly upsertSession: (session: SessionResource) => void;
  readonly requestBrowserConnect: () => void;
  readonly onFollow?: (run: AdminWorkflowRunSnapshot) => void;
  readonly onError: (message: string) => void;
};

export class AdminWorkflowFollowPolicy {
  private static readonly FOLLOW_STATES: readonly string[] = ['starting', 'running', 'awaiting_input'];

  static selectRun(runs: readonly AdminWorkflowRunSnapshot[]): AdminWorkflowRunSnapshot | null {
    return [...runs]
      .filter((run) => AdminWorkflowFollowPolicy.FOLLOW_STATES.includes(run.state))
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))[0] ?? null;
  }

  static signature(run: AdminWorkflowRunSnapshot): string {
    return `${run.id}:${run.sessionId}:${run.state}:${run.updatedAt}`;
  }
}

export class AdminWorkflowSessionFollower {
  private followSignature = '';

  constructor(private readonly options: AdminWorkflowSessionFollowerOptions) {}

  async followRuns(runs: readonly AdminWorkflowRunSnapshot[]): Promise<void> {
    const run = AdminWorkflowFollowPolicy.selectRun(runs);
    if (!run || this.options.getConnectedSessionId() === run.sessionId) return;
    const signature = AdminWorkflowFollowPolicy.signature(run);
    if (signature === this.followSignature) return;
    try {
      const session = this.options.getSessions().find((entry) => entry.id === run.sessionId)
        ?? await this.options.controlClient.getSession(run.sessionId);
      this.options.upsertSession(session);
      this.followSignature = signature;
      this.options.onFollow?.(run);
      this.options.requestBrowserConnect();
    } catch (error) {
      this.options.onError(AdminWorkflowSessionFollower.errorMessage(error));
    }
  }

  private static errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : 'Unexpected workflow follow error';
  }
}
