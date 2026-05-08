import type { SessionResource } from '../api/control-types';

export type AdminSessionSelectionInput = {
  readonly sessions: readonly SessionResource[];
  readonly selectedSession: SessionResource | null;
  readonly pendingSelectedSessionId: string | null;
};

export type AdminSessionSelectionResult = {
  readonly selectedSession: SessionResource | null;
  readonly pendingSelectedSessionId: string | null;
};

export class AdminSessionSelection {
  static afterList(input: AdminSessionSelectionInput): AdminSessionSelectionResult {
    const pending = input.pendingSelectedSessionId
      ? input.sessions.find((session) => session.id === input.pendingSelectedSessionId)
      : null;
    if (pending) {
      return { selectedSession: pending, pendingSelectedSessionId: null };
    }
    if (input.pendingSelectedSessionId && input.selectedSession?.id === input.pendingSelectedSessionId) {
      return {
        selectedSession: input.selectedSession,
        pendingSelectedSessionId: input.pendingSelectedSessionId,
      };
    }
    return {
      selectedSession: this.findCurrentOrFirst(input.sessions, input.selectedSession),
      pendingSelectedSessionId: null,
    };
  }

  private static findCurrentOrFirst(
    sessions: readonly SessionResource[],
    selectedSession: SessionResource | null,
  ): SessionResource | null {
    return sessions.find((session) => session.id === selectedSession?.id) ?? sessions[0] ?? null;
  }
}
