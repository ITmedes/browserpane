<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import type { ControlClient } from '../api/control-client';
  import type { SessionResource } from '../api/control-types';
  import type { McpBridgeConfig } from '../auth/auth-config';
  import BrowserEmbedPanel from '../presentation/BrowserEmbedPanel.svelte';
  import { AdminWorkspaceViewModelBuilder } from '../presentation/admin-workspace-view-model';
  import { SessionViewModelBuilder } from '../presentation/session-view-model';
  import { BrowserSessionConnector } from '../session/browser-session-connector';
  import { DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES, type BrowserSessionConnectPreferences, type LiveBrowserSessionConnection } from '../session/browser-session-types';
  import AdminWorkspaceSidebar from './AdminWorkspaceSidebar.svelte';
  import ResizableWorkspaceLayout from './ResizableWorkspaceLayout.svelte';

  type AdminSessionSurfaceProps = {
    readonly controlClient: ControlClient;
    readonly mcpBridge: McpBridgeConfig | null;
  };
  let { controlClient, mcpBridge }: AdminSessionSurfaceProps = $props();
  let browserConnector = $derived(new BrowserSessionConnector({ controlClient }));
  let liveConnection = $state<LiveBrowserSessionConnection | null>(null);
  let sessions = $state<readonly SessionResource[]>([]);
  let selectedSession = $state<SessionResource | null>(null);
  let sessionsLoading = $state(false);
  let sessionsError = $state<string | null>(null);
  let browserConnecting = $state(false);
  let browserError = $state<string | null>(null);
  let browserStatus = $state('Disconnected');
  let sessionFileCount = $state(0);
  let browserPreferences = $state<BrowserSessionConnectPreferences>({ ...DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES });
  const browserConnected = $derived(Boolean(liveConnection && liveConnection.sessionId === selectedSession?.id));
  const workspaceViewModel = $derived(AdminWorkspaceViewModelBuilder.build({
    browserStatus,
    selectedSessionId: selectedSession?.id ?? null,
    sessionCount: sessions.length,
    fileCount: sessionFileCount,
    connected: browserConnected,
  }));
  const sessionListViewModel = $derived(SessionViewModelBuilder.list({
    sessions,
    selectedSessionId: selectedSession?.id ?? null,
    authenticated: true,
    loading: sessionsLoading,
    error: sessionsError,
  }));
  const sessionDetailViewModel = $derived(SessionViewModelBuilder.detail({
    session: selectedSession,
    connected: browserConnected,
    loading: sessionsLoading,
    error: sessionsError,
  }));

  onMount(() => { void loadSessions(); });
  onDestroy(() => { disconnectBrowser(false); });

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
      selectedSession = await controlClient.createSession();
      setSessionList((await controlClient.listSessions()).sessions);
    } catch (error) {
      sessionsError = errorMessage(error);
    } finally {
      sessionsLoading = false;
    }
  }

  async function refreshSelectedSession(): Promise<void> {
    if (!selectedSession) {
      return;
    }
    sessionsLoading = true;
    try {
      upsertSession(await controlClient.getSession(selectedSession.id));
    } catch (error) {
      sessionsError = errorMessage(error);
    } finally {
      sessionsLoading = false;
    }
  }

  async function runLifecycle(action: 'stop' | 'kill'): Promise<void> {
    if (!selectedSession) {
      return;
    }
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
    if (!selectedSession) {
      return;
    }
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
    selectedSession = next.find((session) => session.id === selectedSession?.id) ?? next[0] ?? null;
  }

  function upsertSession(session: SessionResource): void {
    selectedSession = session;
    sessions = sessions.some((entry) => entry.id === session.id)
      ? sessions.map((entry) => entry.id === session.id ? session : entry)
      : [session, ...sessions];
  }

  function selectSession(sessionId: string): void {
    selectedSession = sessions.find((session) => session.id === sessionId) ?? selectedSession;
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : 'Unexpected admin console error';
  }
</script>

<ResizableWorkspaceLayout>
  {#snippet browser()}
    <BrowserEmbedPanel
      viewModel={workspaceViewModel.browser}
      session={selectedSession}
      connectedSessionId={liveConnection?.sessionId ?? null}
      connecting={browserConnecting}
      error={browserError}
      onConnect={(container) => void connectBrowser(container)}
      onDisconnect={() => disconnectBrowser(true)}
    />
  {/snippet}
  {#snippet sidebar()}
    <AdminWorkspaceSidebar
      {controlClient}
      {selectedSession}
      {sessions}
      {mcpBridge}
      {liveConnection}
      {browserPreferences}
      {browserConnected}
      {workspaceViewModel}
      {sessionListViewModel}
      {sessionDetailViewModel}
      onRefreshSessions={loadSessions}
      onCreateSession={() => void createSession()}
      onSelectSessionId={selectSession}
      onRefreshSelectedSession={refreshSelectedSession}
      onStopSession={() => void runLifecycle('stop')}
      onKillSession={() => void runLifecycle('kill')}
      onFileCountChange={(count) => { sessionFileCount = count; }}
      onBrowserPreferencesChange={(next) => { browserPreferences = next; }}
    />
  {/snippet}
</ResizableWorkspaceLayout>
