<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import type { ControlClient } from '../api/control-client';
  import type { SessionResource } from '../api/control-types';
  import BrowserEmbedPanel from '../presentation/BrowserEmbedPanel.svelte';
  import SessionDetailPanel from '../presentation/SessionDetailPanel.svelte';
  import SessionListPanel from '../presentation/SessionListPanel.svelte';
  import { BrowserSessionConnector } from '../session/browser-session-connector';
  import type { LiveBrowserSessionConnection } from '../session/browser-session-types';
  import SessionFilesSurface from './SessionFilesSurface.svelte';

  type AdminSessionSurfaceProps = {
    readonly controlClient: ControlClient;
  };

  let { controlClient }: AdminSessionSurfaceProps = $props();
  let browserConnector = $derived(new BrowserSessionConnector({ controlClient }));
  let liveConnection = $state<LiveBrowserSessionConnection | null>(null);
  let sessions = $state<readonly SessionResource[]>([]);
  let selectedSession = $state<SessionResource | null>(null);
  let sessionsLoading = $state(false);
  let sessionsError = $state<string | null>(null);
  let browserConnecting = $state(false);
  let browserError = $state<string | null>(null);
  let browserStatus = $state('Disconnected');

  onMount(() => {
    void loadSessions();
  });

  onDestroy(() => {
    disconnectBrowser(false);
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
      liveConnection = await browserConnector.connect(selectedSession, container);
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

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : 'Unexpected admin console error';
  }
</script>

<SessionListPanel
  {sessions}
  selectedSessionId={selectedSession?.id ?? null}
  authenticated={true}
  loading={sessionsLoading}
  error={sessionsError}
  onRefresh={() => void loadSessions()}
  onCreateSession={() => void createSession()}
  onSelectSession={(session) => {
    selectedSession = session;
  }}
/>

<SessionDetailPanel
  session={selectedSession}
  loading={sessionsLoading}
  error={sessionsError}
  connected={Boolean(liveConnection && liveConnection.sessionId === selectedSession?.id)}
  onRefresh={() => void refreshSelectedSession()}
  onStop={() => void runLifecycle('stop')}
  onKill={() => void runLifecycle('kill')}
/>

<SessionFilesSurface {controlClient} session={selectedSession} />

<BrowserEmbedPanel
  session={selectedSession}
  connectedSessionId={liveConnection?.sessionId ?? null}
  connecting={browserConnecting}
  status={browserStatus}
  error={browserError}
  onConnect={(container) => void connectBrowser(container)}
  onDisconnect={() => disconnectBrowser(true)}
/>
