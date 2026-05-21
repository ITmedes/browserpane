<script lang="ts">
  import { onMount } from 'svelte';
  import type { AdminEventClient, AdminEventConnectionStatus } from '../api/admin-event-client';
  import type {
    AdminMcpDelegationSnapshot,
    AdminRecordingsSnapshot,
    AdminSessionFilesSnapshot,
    AdminWorkflowRunSnapshot,
  } from '../api/admin-event-snapshots';
  import type { ControlClient } from '../api/control-client';
  import type {
    BrowserContextResource,
    CreateBrowserContextCommand,
    CreateSessionCommand,
    SessionResource,
    SessionTemplateResource,
  } from '../api/control-types';
  import type { WorkflowClient } from '../api/workflow-client';
  import type { McpBridgeConfig } from '../auth/auth-config';
  import BrowserEmbedPanel from '../presentation/BrowserEmbedPanel.svelte';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import type { AdminLogEntry } from '../presentation/logs-view-model';
  import { AdminWorkspaceViewModelBuilder } from '../presentation/admin-workspace-view-model';
  import { SessionViewModelBuilder } from '../presentation/session-view-model';
  import { BrowserSessionConnector } from '../session/browser-session-connector';
  import { DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES, type BrowserSessionConnectPreferences, type LiveBrowserSessionConnection } from '../session/browser-session-types';
  import AdminWorkspaceTabs from './AdminWorkspaceTabs.svelte';
  import { AdminLogEntryFactory } from './admin-log-entries';
  import { AdminSessionSelection } from './admin-session-selection';
  import {
    eventStreamStatusMessage,
    globalAdminMessage,
    mcpDelegationSnapshotMessage,
    recordingsSnapshotMessage,
    selectedSessionDiffMessage,
    sessionFilesSnapshotMessage,
    shortAdminId,
    workflowFollowMessage,
    workflowRunsSnapshotMessage,
  } from './admin-feedback-notifications';
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
  let sessionTemplates = $state<readonly SessionTemplateResource[]>([]);
  let browserContexts = $state<readonly BrowserContextResource[]>([]);
  let selectedSession = $state<SessionResource | null>(null);
  let sessionsLoading = $state(false);
  let sessionsError = $state<string | null>(null);
  let templatesLoading = $state(false);
  let browserContextsLoading = $state(false);
  let templateError = $state<string | null>(null);
  let browserContextError = $state<string | null>(null);
  let globalMessage = $state<AdminMessageFeedback | null>(null);
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
  let sessionSnapshotSeen = false;
  let eventStreamStatus: AdminEventConnectionStatus | null = null;
  let sessionFileCounts: ReadonlyMap<string, number> = new Map();
  let recordingSnapshots: ReadonlyMap<string, AdminRecordingsSnapshot> = new Map();
  let mcpSnapshots: ReadonlyMap<string, AdminMcpDelegationSnapshot> = new Map();
  let workflowRunStates: ReadonlyMap<string, string> = new Map();
  const browserConnected = $derived(Boolean(liveConnection && liveConnection.sessionId === selectedSession?.id));
  const workflowFollower = $derived(new AdminWorkflowSessionFollower({
    controlClient, getSessions: () => sessions, getConnectedSessionId: () => liveConnection?.sessionId ?? null,
    upsertSession, requestBrowserConnect,
    onFollow: notifyWorkflowFollow,
    onError: (message) => {
      sessionsError = message;
      showGlobalMessage('error', 'Workflow follow failed', message);
    },
  }));
  const workspaceViewModel = $derived(AdminWorkspaceViewModelBuilder.build({
    browserStatus, selectedSessionId: selectedSession?.id ?? null,
    sessionCount: sessions.length, browserContextCount: browserContexts.length,
    fileCount: sessionFileCount, connected: browserConnected,
  }));
  const sessionListViewModel = $derived(SessionViewModelBuilder.list({
    sessions, sessionTemplates, browserContexts, selectedSessionId: selectedSession?.id ?? null,
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
      onSessions: (next) => setSessionList(next, 'event'),
      onLoadingChange: (loading) => { sessionsLoading = loading; },
      onError: (error) => {
        sessionsError = error;
        if (error) {
          showGlobalMessage('error', 'Admin event stream', error);
        }
      },
      onLog: appendLog,
      onConnectionStatus: handleEventStreamStatus,
      onSessionFilesSnapshot: handleSessionFilesSnapshot,
      onRecordingsSnapshot: handleRecordingsSnapshot,
      onMcpDelegationSnapshot: handleMcpDelegationSnapshot,
      onWorkflowRunsSnapshot: (runs) => {
        handleWorkflowRunsSnapshot(runs);
        void workflowFollower.followRuns(runs);
      },
    });
    void loadSessions();
    void loadSessionTemplates();
    void loadBrowserContexts();
    return () => { subscription.close(); disconnectBrowser(false); };
  });
  async function loadSessionTemplates(showFeedback = false): Promise<void> {
    templatesLoading = true;
    templateError = null;
    try {
      sessionTemplates = (await controlClient.listSessionTemplates()).templates;
      if (showFeedback) {
        showGlobalMessage(
          'success',
          'Template catalog refreshed',
          `${sessionTemplates.length} template${sessionTemplates.length === 1 ? '' : 's'} refreshed.`,
        );
      }
    } catch (error) {
      templateError = errorMessage(error);
      showGlobalMessage('warning', 'Template catalog unavailable', templateError);
    } finally {
      templatesLoading = false;
    }
  }
  async function loadBrowserContexts(showFeedback = false): Promise<void> {
    browserContextsLoading = true;
    browserContextError = null;
    try {
      browserContexts = (await controlClient.listBrowserContexts()).contexts;
      if (showFeedback) {
        showGlobalMessage(
          'success',
          'Browser contexts refreshed',
          `${browserContexts.length} context${browserContexts.length === 1 ? '' : 's'} refreshed.`,
        );
      }
    } catch (error) {
      browserContextError = errorMessage(error);
      showGlobalMessage('warning', 'Browser context catalog unavailable', browserContextError);
    } finally {
      browserContextsLoading = false;
    }
  }
  async function loadSessions(showFeedback = false): Promise<void> {
    sessionsLoading = true;
    sessionsError = null;
    try {
      const nextSessions = (await controlClient.listSessions()).sessions;
      setSessionList(nextSessions, 'manual');
      if (showFeedback) {
        showGlobalMessage('success', 'Sessions refreshed', `${nextSessions.length} session${nextSessions.length === 1 ? '' : 's'} refreshed.`);
      }
    } catch (error) {
      sessionsError = errorMessage(error);
      if (showFeedback) {
        showGlobalMessage('error', 'Session refresh failed', sessionsError);
      }
    } finally {
      sessionsLoading = false;
    }
  }
  async function createSession(command: CreateSessionCommand = {}): Promise<void> {
    sessionsLoading = true;
    sessionsError = null;
    try {
      const created = await controlClient.createSession(command);
      pendingSelectedSessionId = created.id;
      upsertSession(created);
      setSessionList((await controlClient.listSessions()).sessions, 'local');
      showGlobalMessage('success', 'Session created', `Created session ${shortAdminId(created.id)}.`);
      void loadBrowserContexts(false);
      requestBrowserConnect();
    } catch (error) {
      sessionsError = errorMessage(error);
      showGlobalMessage('error', 'Session create failed', sessionsError);
    } finally {
      sessionsLoading = false;
    }
  }
  async function createBrowserContext(command: CreateBrowserContextCommand): Promise<BrowserContextResource> {
    browserContextsLoading = true;
    browserContextError = null;
    try {
      const created = await controlClient.createBrowserContext(command);
      browserContexts = [created, ...browserContexts.filter((context) => context.id !== created.id)];
      showGlobalMessage('success', 'Browser context saved', `Saved reusable context ${shortAdminId(created.id)}.`);
      return created;
    } catch (error) {
      browserContextError = errorMessage(error);
      showGlobalMessage('error', 'Browser context save failed', browserContextError);
      throw error;
    } finally {
      browserContextsLoading = false;
    }
  }
  async function deleteBrowserContext(contextId: string): Promise<void> {
    browserContextsLoading = true;
    browserContextError = null;
    try {
      const deleted = await controlClient.deleteBrowserContext(contextId);
      browserContexts = browserContexts.map((context) => context.id === deleted.id ? deleted : context);
      showGlobalMessage('success', 'Browser context deleted', `Deleted context ${shortAdminId(deleted.id)}.`);
      void loadSessions(false);
    } catch (error) {
      browserContextError = errorMessage(error);
      showGlobalMessage('error', 'Browser context delete failed', browserContextError);
      throw error;
    } finally {
      browserContextsLoading = false;
    }
  }
  async function refreshSelectedSession(options: { readonly showGlobalError?: boolean } = {}): Promise<void> {
    if (!selectedSession) return;
    sessionsLoading = true;
    sessionsError = null;
    try { upsertSession(await controlClient.getSession(selectedSession.id)); }
    catch (error) {
      sessionsError = errorMessage(error);
      if (options.showGlobalError) {
        showGlobalMessage('warning', 'Session refresh failed', sessionsError);
      }
      throw error;
    }
    finally { sessionsLoading = false; }
  }
  async function refreshSelectedSessionInBackground(): Promise<void> {
    try {
      await refreshSelectedSession({ showGlobalError: true });
    } catch {
      // The foreground action already succeeded; keep its state instead of
      // turning a follow-up refresh failure into a failed action.
    }
  }
  async function runLifecycle(action: 'release' | 'stop' | 'kill'): Promise<void> {
    if (!selectedSession) return;
    sessionsLoading = true;
    sessionsError = null;
    try {
      const updated = action === 'release'
        ? await controlClient.releaseSessionRuntime(selectedSession.id)
        : action === 'stop'
          ? await controlClient.stopSession(selectedSession.id)
          : await controlClient.killSession(selectedSession.id);
      upsertSession(updated);
    } catch (error) {
      sessionsError = errorMessage(error);
      throw error;
    } finally {
      sessionsLoading = false;
    }
  }
  async function connectBrowser(container: HTMLElement): Promise<void> {
    const session = selectedSession;
    if (!session) return;
    disconnectBrowser(false);
    browserConnecting = true;
    browserError = null;
    browserStatus = `Connecting to ${session.id}`;
    showGlobalMessage('loading', 'Browser connection', `Connecting to session ${shortAdminId(session.id)}...`);
    try {
      const connection = await browserConnector.connect(session, container, browserPreferences);
      if (selectedSession?.id !== session.id) {
        connection.handle.disconnect();
        browserStatus = 'Disconnected';
        return;
      }
      liveConnection = connection;
      browserStatus = `Connected to ${session.id}`;
      showGlobalMessage('success', 'Browser connected', `Connected to session ${shortAdminId(session.id)}.`);
      void refreshSelectedSessionInBackground();
    } catch (error) {
      browserError = errorMessage(error);
      browserStatus = 'Connection failed';
      showGlobalMessage('error', 'Browser connection failed', browserError);
    } finally {
      browserConnecting = false;
    }
  }
  function disconnectBrowser(refreshAfterDisconnect = false): void {
    const hadLiveConnection = Boolean(liveConnection);
    const disconnectedSessionId = liveConnection?.sessionId ?? null;
    liveConnection?.handle.disconnect();
    liveConnection = null;
    browserStatus = 'Disconnected';
    if (hadLiveConnection) {
      showGlobalMessage(
        'info',
        'Browser disconnected',
        disconnectedSessionId ? `Disconnected from session ${shortAdminId(disconnectedSessionId)}.` : 'Browser disconnected.',
      );
    }
    if (hadLiveConnection && refreshAfterDisconnect) {
      window.setTimeout(() => void refreshSelectedSessionInBackground(), 250);
    }
  }
  function setSessionList(next: readonly SessionResource[], source: 'event' | 'manual' | 'local' = 'manual'): void {
    const previousSelected = selectedSession;
    sessions = next;
    const selection = AdminSessionSelection.afterList({ sessions, selectedSession, pendingSelectedSessionId });
    selectedSession = selection.selectedSession;
    pendingSelectedSessionId = selection.pendingSelectedSessionId;
    if (source === 'event') {
      notifySelectedSessionDiff(previousSelected, selectedSession);
      sessionSnapshotSeen = true;
    }
  }
  function upsertSession(session: SessionResource): void {
    selectedSession = session;
    sessions = sessions.some((entry) => entry.id === session.id)
      ? sessions.map((entry) => entry.id === session.id ? session : entry)
      : [session, ...sessions];
  }
  function selectSession(sessionId: string): void {
    const nextSession = sessions.find((session) => session.id === sessionId) ?? null;
    pendingSelectedSessionId = null;
    if (!nextSession) {
      showGlobalMessage('warning', 'Session selection failed', `Session ${shortAdminId(sessionId)} is not visible.`);
      return;
    }
    const previousLiveSessionId = liveConnection?.sessionId ?? null;
    const disconnectForSwitch = Boolean(previousLiveSessionId && previousLiveSessionId !== nextSession.id);
    if (disconnectForSwitch) {
      liveConnection?.handle.disconnect();
      liveConnection = null;
      browserError = null;
      browserStatus = 'Disconnected';
    }
    selectedSession = nextSession;
    if (disconnectForSwitch && previousLiveSessionId) {
      showGlobalMessage(
        'info',
        'Session switched',
        `Disconnected from session ${shortAdminId(previousLiveSessionId)} and selected session ${shortAdminId(nextSession.id)}.`,
      );
      return;
    }
    showGlobalMessage('info', 'Session selected', `Selected session ${shortAdminId(selectedSession?.id ?? sessionId)}.`);
  }
  function requestBrowserConnect(): void { browserConnectRequestVersion += 1; }
  function appendLog(entry: AdminLogEntry): void { logEntries = AdminLogEntryFactory.append(logEntries, entry); }
  function handleEventStreamStatus(status: AdminEventConnectionStatus): void {
    const message = eventStreamStatusMessage(eventStreamStatus, status);
    eventStreamStatus = status;
    showGlobalNotification(message);
  }
  function handleSessionFilesSnapshot(snapshot: readonly AdminSessionFilesSnapshot[]): void {
    sessionFilesRefreshVersion += 1;
    const result = sessionFilesSnapshotMessage(selectedSession?.id ?? null, snapshot, sessionFileCounts);
    sessionFileCounts = result.counts;
    showGlobalNotification(result.message);
  }
  function handleRecordingsSnapshot(snapshot: readonly AdminRecordingsSnapshot[]): void {
    recordingsRefreshVersion += 1;
    const result = recordingsSnapshotMessage(selectedSession?.id ?? null, snapshot, recordingSnapshots);
    recordingSnapshots = result.snapshots;
    showGlobalNotification(result.message);
  }
  function handleMcpDelegationSnapshot(snapshot: readonly AdminMcpDelegationSnapshot[]): void {
    mcpDelegationRefreshVersion += 1;
    const result = mcpDelegationSnapshotMessage(selectedSession?.id ?? null, snapshot, mcpSnapshots);
    mcpSnapshots = result.snapshots;
    showGlobalNotification(result.message);
  }
  function handleWorkflowRunsSnapshot(runs: readonly AdminWorkflowRunSnapshot[]): void {
    const result = workflowRunsSnapshotMessage(selectedSession?.id ?? null, runs, workflowRunStates);
    workflowRunStates = result.states;
    showGlobalNotification(result.message);
  }
  function notifyWorkflowFollow(run: AdminWorkflowRunSnapshot): void {
    showGlobalNotification(workflowFollowMessage(run));
  }
  function notifySelectedSessionDiff(previous: SessionResource | null, current: SessionResource | null): void {
    showGlobalNotification(selectedSessionDiffMessage(previous, current, sessionSnapshotSeen));
  }
  function showGlobalMessage(
    variant: AdminMessageFeedback['variant'],
    title: string,
    message: string,
  ): void {
    globalMessage = globalAdminMessage(variant, title, message);
  }
  function showGlobalNotification(message: AdminMessageFeedback | null): void {
    if (message) {
      globalMessage = message;
    }
  }
  function dismissGlobalMessage(): void {
    globalMessage = null;
  }
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
      {controlClient} {workflowClient} {selectedSession} {sessionTemplates} {templatesLoading} {templateError} {mcpBridge} {liveConnection}
      {sessions} {browserContexts} {browserContextsLoading} {browserContextError}
      {browserPreferences} {browserConnected} {workspaceViewModel} {sessionListViewModel} {logEntries} {globalMessage}
      {sessionFilesRefreshVersion} {recordingsRefreshVersion} {mcpDelegationRefreshVersion} onRefreshSessions={loadSessions}
      onCreateSession={(command) => void createSession(command)} onJoinSelectedSession={requestBrowserConnect}
      onCreateBrowserContext={createBrowserContext}
      onRefreshBrowserContexts={loadBrowserContexts}
      onDeleteBrowserContext={deleteBrowserContext}
      onSelectSessionId={selectSession} onRefreshSelectedSession={refreshSelectedSession}
      onReleaseSessionRuntime={() => runLifecycle('release')}
      onStopSession={() => runLifecycle('stop')} onKillSession={() => runLifecycle('kill')} onDisconnectEmbeddedBrowser={() => disconnectBrowser(false)}
      onFileCountChange={(count) => { sessionFileCount = count; }}
      onClearLogs={() => { logEntries = []; }}
      onBrowserPreferencesChange={(next) => { browserPreferences = next; }}
      onDismissGlobalMessage={dismissGlobalMessage}
    />
    {/snippet}
  </BrowserWorkspaceOverlayLayout>
</section>
