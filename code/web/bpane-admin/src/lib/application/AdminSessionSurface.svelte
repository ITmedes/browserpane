<script lang="ts">
  import { onMount } from 'svelte';
  import type { AdminEventClient } from '../api/admin-event-client';
  import type { ControlClient } from '../api/control-client';
  import type { SessionResource } from '../api/control-types';
  import type { WorkflowClient } from '../api/workflow-client';
  import type { McpBridgeConfig } from '../auth/auth-config';
  import BrowserEmbedPanel from '../presentation/BrowserEmbedPanel.svelte';
  import type { AdminLogEntry } from '../presentation/logs-view-model';
  import { AdminWorkspaceViewModelBuilder } from '../presentation/admin-workspace-view-model';
  import { SessionViewModelBuilder } from '../presentation/session-view-model';
  import { BrowserSessionConnector } from '../session/browser-session-connector';
  import { DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES, type BrowserSessionConnectPreferences, type LiveBrowserSessionConnection } from '../session/browser-session-types';
  import AdminWorkspaceTabs from './AdminWorkspaceTabs.svelte';
  import { AdminLogEntryFactory } from './admin-log-entries';
  import { AdminSessionSelection } from './admin-session-selection';
  import { subscribeAdminSessionEvents } from './admin-session-event-sync';
  import { AdminWorkflowSessionFollower } from './admin-workflow-follow';
  import BrowserWorkspaceOverlayLayout from './BrowserWorkspaceOverlayLayout.svelte';
  type AdminSessionSurfaceProps = {
    readonly controlClient: ControlClient; readonly adminEventClient: AdminEventClient; readonly workflowClient: WorkflowClient;
    readonly mcpBridge: McpBridgeConfig | null;
    readonly adminOpen: boolean;
    readonly onAdminOpenChange: (open: boolean) => void;
  };
  let { controlClient, adminEventClient, workflowClient, mcpBridge, adminOpen, onAdminOpenChange }: AdminSessionSurfaceProps = $props();
  let browserConnector = $derived(new BrowserSessionConnector({ controlClient }));
  let liveConnection = $state<LiveBrowserSessionConnection | null>(null);
  let sessions = $state<readonly SessionResource[]>([]);
  let selectedSession = $state<SessionResource | null>(null);
  let sessionsLoading = $state(false);
  let sessionsError = $state<string | null>(null);
  let pendingSelectedSessionId = $state<string | null>(null);
  let browserConnecting = $state(false);
  let browserError = $state<string | null>(null);
  let browserStatus = $state('Disconnected');
  let browserConnectRequestVersion = $state(0);
  let sessionFileCount = $state(0);
  let sessionFilesRefreshVersion = $state(0);
  let recordingsRefreshVersion = $state(0);
  let mcpDelegationRefreshVersion = $state(0);
  let logEntries = $state<readonly AdminLogEntry[]>([]);
  let lastLogSignature = $state('');
  let browserPreferences = $state<BrowserSessionConnectPreferences>({ ...DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES });
  const browserConnected = $derived(Boolean(liveConnection && liveConnection.sessionId === selectedSession?.id));
  const workflowFollower = $derived(new AdminWorkflowSessionFollower({
    controlClient, getSessions: () => sessions, getConnectedSessionId: () => liveConnection?.sessionId ?? null,
    upsertSession, requestBrowserConnect,
    onError: (message) => { sessionsError = message; },
  }));
  const workspaceViewModel = $derived(AdminWorkspaceViewModelBuilder.build({
    browserStatus, selectedSessionId: selectedSession?.id ?? null,
    sessionCount: sessions.length, fileCount: sessionFileCount, connected: browserConnected,
  }));
  const sessionListViewModel = $derived(SessionViewModelBuilder.list({
    sessions, selectedSessionId: selectedSession?.id ?? null,
    authenticated: true, loading: sessionsLoading, error: sessionsError,
  }));
  $effect(() => {
    const signature = `${selectedSession?.id ?? 'none'}:${selectedSession?.state ?? 'none'}:${browserConnected}:${sessions.length}`;
    if (signature !== lastLogSignature) {
      lastLogSignature = signature;
      appendLog(AdminLogEntryFactory.fromUiState({ selectedSession, browserConnected, sessionCount: sessions.length }));
    }
  });
  onMount(() => {
    const subscription = subscribeAdminSessionEvents(adminEventClient, {
      onSessions: setSessionList,
      onLoadingChange: (loading) => { sessionsLoading = loading; },
      onError: (error) => { sessionsError = error; },
      onLog: appendLog,
      onSessionFilesSnapshot: () => { sessionFilesRefreshVersion += 1; },
      onRecordingsSnapshot: () => { recordingsRefreshVersion += 1; },
      onMcpDelegationSnapshot: () => { mcpDelegationRefreshVersion += 1; },
      onWorkflowRunsSnapshot: (runs) => void workflowFollower.followRuns(runs),
    });
    void loadSessions();
    return () => { subscription.close(); disconnectBrowser(false); };
  });
  async function loadSessions(): Promise<void> {
    sessionsLoading = true;
    sessionsError = null;
    try {
      setSessionList((await controlClient.listSessions()).sessions);
    } catch (error) {
      sessionsError = errorMessage(error);
    } finally {
      sessionsLoading = false;
    }
  }
  async function createSession(): Promise<void> {
    sessionsLoading = true;
    sessionsError = null;
    try {
      const created = await controlClient.createSession();
      pendingSelectedSessionId = created.id;
      upsertSession(created);
      setSessionList((await controlClient.listSessions()).sessions);
      requestBrowserConnect();
    } catch (error) {
      sessionsError = errorMessage(error);
    } finally {
      sessionsLoading = false;
    }
  }
  async function refreshSelectedSession(): Promise<void> {
    if (!selectedSession) return;
    sessionsLoading = true;
    try { upsertSession(await controlClient.getSession(selectedSession.id)); }
    catch (error) { sessionsError = errorMessage(error); }
    finally { sessionsLoading = false; }
  }
  async function runLifecycle(action: 'stop' | 'kill'): Promise<void> {
    if (!selectedSession) return;
    sessionsLoading = true;
    sessionsError = null;
    try {
      const updated = action === 'stop'
        ? await controlClient.stopSession(selectedSession.id)
        : await controlClient.killSession(selectedSession.id);
      upsertSession(updated);
    } catch (error) {
      sessionsError = errorMessage(error);
    } finally {
      sessionsLoading = false;
    }
  }
  async function connectBrowser(container: HTMLElement): Promise<void> {
    if (!selectedSession) return;
    disconnectBrowser(false);
    browserConnecting = true;
    browserError = null;
    browserStatus = `Connecting to ${selectedSession.id}`;
    try {
      liveConnection = await browserConnector.connect(selectedSession, container, browserPreferences);
      browserStatus = `Connected to ${selectedSession.id}`;
      await refreshSelectedSession();
    } catch (error) {
      browserError = errorMessage(error);
      browserStatus = 'Connection failed';
    } finally {
      browserConnecting = false;
    }
  }
  function disconnectBrowser(refreshAfterDisconnect = false): void {
    const hadLiveConnection = Boolean(liveConnection);
    liveConnection?.handle.disconnect();
    liveConnection = null;
    browserStatus = 'Disconnected';
    if (hadLiveConnection && refreshAfterDisconnect) {
      window.setTimeout(() => void refreshSelectedSession(), 250);
    }
  }
  function setSessionList(next: readonly SessionResource[]): void {
    sessions = next;
    const selection = AdminSessionSelection.afterList({ sessions, selectedSession, pendingSelectedSessionId });
    selectedSession = selection.selectedSession;
    pendingSelectedSessionId = selection.pendingSelectedSessionId;
  }
  function upsertSession(session: SessionResource): void {
    selectedSession = session;
    sessions = sessions.some((entry) => entry.id === session.id)
      ? sessions.map((entry) => entry.id === session.id ? session : entry)
      : [session, ...sessions];
  }
  function selectSession(sessionId: string): void {
    pendingSelectedSessionId = null;
    selectedSession = sessions.find((session) => session.id === sessionId) ?? selectedSession;
  }
  function requestBrowserConnect(): void { browserConnectRequestVersion += 1; }
  function appendLog(entry: AdminLogEntry): void { logEntries = AdminLogEntryFactory.append(logEntries, entry); }
  function errorMessage(error: unknown): string { return error instanceof Error ? error.message : 'Unexpected admin console error'; }
</script>
<section class="relative h-[calc(100dvh-96px)] min-h-[520px]">
  <main class="h-full min-w-0">
    <BrowserEmbedPanel
      viewModel={workspaceViewModel.browser} session={selectedSession}
      connectedSessionId={liveConnection?.sessionId ?? null} connecting={browserConnecting} error={browserError}
      autoConnectVersion={browserConnectRequestVersion}
      onConnect={(container) => void connectBrowser(container)} onDisconnect={() => disconnectBrowser(true)}
    />
  </main>
  <BrowserWorkspaceOverlayLayout {adminOpen} {onAdminOpenChange}>
    {#snippet admin()}
    <AdminWorkspaceTabs
      {controlClient} {workflowClient} {selectedSession} {sessions} {mcpBridge} {liveConnection}
      {browserPreferences} {browserConnected} {workspaceViewModel} {sessionListViewModel} {logEntries}
      {sessionFilesRefreshVersion} {recordingsRefreshVersion} {mcpDelegationRefreshVersion} onRefreshSessions={loadSessions}
      onCreateSession={() => void createSession()} onJoinSelectedSession={requestBrowserConnect}
      onSelectSessionId={selectSession} onRefreshSelectedSession={refreshSelectedSession}
      onStopSession={() => void runLifecycle('stop')} onKillSession={() => void runLifecycle('kill')} onDisconnectEmbeddedBrowser={() => disconnectBrowser(false)}
      onFileCountChange={(count) => { sessionFileCount = count; }}
      onClearLogs={() => { logEntries = []; }}
      onBrowserPreferencesChange={(next) => { browserPreferences = next; }}
    />
    {/snippet}
  </BrowserWorkspaceOverlayLayout>
</section>
