<script lang="ts">
  import type { ControlClient } from '../api/control-client';
  import type { SessionResource } from '../api/control-types';
  import type { SessionStatus } from '../api/session-status-types';
  import SessionDetailPanel from '../presentation/SessionDetailPanel.svelte';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import { SessionViewModelBuilder } from '../presentation/session-view-model';

  type SessionLifecycleSurfaceProps = {
    readonly controlClient: ControlClient;
    readonly selectedSession: SessionResource | null;
    readonly connected: boolean;
    readonly resourceLoading: boolean;
    readonly onRefreshSelectedSession: () => Promise<void>;
    readonly onStopSession: () => Promise<void>;
    readonly onKillSession: () => Promise<void>;
    readonly onDisconnectEmbeddedBrowser: () => void;
  };

  let {
    controlClient,
    selectedSession,
    connected,
    resourceLoading,
    onRefreshSelectedSession,
    onStopSession,
    onKillSession,
    onDisconnectEmbeddedBrowser,
  }: SessionLifecycleSurfaceProps = $props();

  let status = $state<SessionStatus | null>(null);
  let lifecycleSessionId = $state<string | null>(null);
  let statusSessionId = $state<string | null>(null);
  let requestedSessionId = $state<string | null>(null);
  let statusLoading = $state(false);
  let statusError = $state<string | null>(null);
  let feedback = $state<AdminMessageFeedback | null>(null);
  const visibleStatus = $derived(statusSessionId === selectedSession?.id ? status : null);
  const loading = $derived(resourceLoading || statusLoading);
  const viewModel = $derived(SessionViewModelBuilder.detail({
    session: selectedSession,
    status: visibleStatus,
    connected,
    loading,
    error: statusError,
  }));

  $effect(() => {
    const sessionId = selectedSession?.id ?? null;
    if (sessionId !== lifecycleSessionId) {
      lifecycleSessionId = sessionId;
      statusError = null;
      feedback = null;
    }
    if (!sessionId) {
      status = null;
      statusSessionId = null;
      requestedSessionId = null;
      statusError = null;
      feedback = null;
      return;
    }
    if (sessionId !== statusSessionId && sessionId !== requestedSessionId) {
      void refreshStatusFor(sessionId);
    }
  });

  async function refreshPanel(): Promise<void> {
    feedback = loadingFeedback('Refreshing session status...');
    try {
      await onRefreshSelectedSession();
      await refreshStatus();
      feedback = successFeedback('Session status refreshed.');
    } catch (error) {
      statusError = errorMessage(error);
      feedback = null;
    }
  }

  async function refreshStatus(): Promise<void> {
    const sessionId = selectedSession?.id ?? null;
    if (sessionId) {
      await refreshStatusFor(sessionId);
    }
  }

  async function disconnectConnection(connectionId: number): Promise<void> {
    const sessionId = selectedSession?.id ?? null;
    if (!sessionId) {
      return;
    }
    await runDisconnect(
      sessionId,
      `Disconnecting client #${connectionId}...`,
      `Disconnected client #${connectionId}.`,
      () => controlClient.disconnectSessionConnection(sessionId, connectionId),
    );
  }

  async function disconnectAllConnections(): Promise<void> {
    const sessionId = selectedSession?.id ?? null;
    if (!sessionId) {
      return;
    }
    await runDisconnect(
      sessionId,
      'Disconnecting all live clients...',
      'Disconnected all live clients.',
      () => controlClient.disconnectAllSessionConnections(sessionId),
    );
  }

  async function runDisconnect(
    sessionId: string,
    progressMessage: string,
    successMessage: string,
    action: () => Promise<SessionStatus>,
  ): Promise<void> {
    statusLoading = true;
    statusError = null;
    feedback = loadingFeedback(progressMessage);
    try {
      const nextStatus = await action();
      applyStatus(sessionId, nextStatus);
      syncEmbeddedBrowserAfterDisconnect(nextStatus);
      await onRefreshSelectedSession();
      feedback = successFeedback(successMessage);
    } catch (error) {
      statusError = errorMessage(error);
      feedback = null;
    } finally {
      statusLoading = false;
    }
  }

  async function runLifecycleAction(
    progressMessage: string,
    successMessage: string,
    action: () => Promise<void>,
  ): Promise<void> {
    feedback = loadingFeedback(progressMessage);
    try {
      await action();
      await refreshStatus();
      feedback = successFeedback(successMessage);
    } catch (error) {
      statusError = errorMessage(error);
      feedback = null;
    }
  }

  async function refreshStatusFor(sessionId: string): Promise<void> {
    requestedSessionId = sessionId;
    statusLoading = true;
    statusError = null;
    try {
      applyStatus(sessionId, await controlClient.getSessionStatus(sessionId));
    } catch (error) {
      statusError = errorMessage(error);
    } finally {
      if (requestedSessionId === sessionId) {
        requestedSessionId = null;
        statusLoading = false;
      }
    }
  }

  function applyStatus(sessionId: string, nextStatus: SessionStatus): void {
    status = nextStatus;
    statusSessionId = sessionId;
  }

  function syncEmbeddedBrowserAfterDisconnect(nextStatus: SessionStatus): void {
    if (connected && nextStatus.connection_counts.total_clients === 0) {
      onDisconnectEmbeddedBrowser();
    }
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : 'Unexpected session status error';
  }

  function loadingFeedback(message: string): AdminMessageFeedback {
    return { variant: 'loading', title: 'Lifecycle operation', message, testId: 'session-lifecycle-message' };
  }

  function successFeedback(message: string): AdminMessageFeedback {
    return { variant: 'success', title: 'Lifecycle updated', message, testId: 'session-lifecycle-message' };
  }
</script>

<SessionDetailPanel
  {viewModel}
  {feedback}
  onRefresh={() => void refreshPanel()}
  onStop={() => void runLifecycleAction('Stopping selected session...', 'Selected session stopped.', onStopSession)}
  onKill={() => void runLifecycleAction('Killing selected session...', 'Selected session was force killed.', onKillSession)}
  onDisconnectConnection={(connectionId) => void disconnectConnection(connectionId)}
  onDisconnectAll={() => void disconnectAllConnections()}
/>
