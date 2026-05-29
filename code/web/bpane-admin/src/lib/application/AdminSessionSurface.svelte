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
    CloneBrowserContextCommand,
    CreateBrowserContextCommand,
    CreateEgressProfileCommand,
    CreateSessionCommand,
    EgressDiagnosticsResource,
    EgressProfileResource,
    ImportBrowserContextCommand,
    ProjectResource,
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
  import { ensureLocalEgressPresets } from './local-egress-presets';
  import { saveBlob } from './recording-downloads';
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
  let projects = $state<readonly ProjectResource[]>([]);
  let browserContexts = $state<readonly BrowserContextResource[]>([]);
  let egressProfiles = $state<readonly EgressProfileResource[]>([]);
  let selectedSession = $state<SessionResource | null>(null);
  let sessionsLoading = $state(false);
  let sessionsError = $state<string | null>(null);
  let templatesLoading = $state(false);
  let projectsLoading = $state(false);
  let browserContextsLoading = $state(false);
  let egressProfilesLoading = $state(false);
  let cloningBrowserContextId = $state<string | null>(null);
  let exportingBrowserContextId = $state<string | null>(null);
  let importingBrowserContext = $state(false);
  let templateError = $state<string | null>(null);
  let projectError = $state<string | null>(null);
  let browserContextError = $state<string | null>(null);
  let egressProfileError = $state<string | null>(null);
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
    egressProfileCount: egressProfiles.length,
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
    void loadProjects();
    void loadSessionTemplates();
    void loadBrowserContexts();
    void loadEgressProfiles();
    return () => { subscription.close(); disconnectBrowser(false); };
  });
  async function loadProjects(showFeedback = false): Promise<void> {
    projectsLoading = true;
    projectError = null;
    try {
      projects = (await controlClient.listProjects()).projects;
      if (showFeedback) {
        showGlobalMessage(
          'success',
          'Projects refreshed',
          `${projects.length} project${projects.length === 1 ? '' : 's'} refreshed.`,
        );
      }
    } catch (error) {
      projectError = errorMessage(error);
      showGlobalMessage('warning', 'Project catalog unavailable', projectError);
    } finally {
      projectsLoading = false;
    }
  }
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
  async function loadEgressProfiles(showFeedback = false): Promise<void> {
    egressProfilesLoading = true;
    egressProfileError = null;
    try {
      const listed = (await controlClient.listEgressProfiles()).profiles;
      const localPresets = await ensureLocalEgressPresets(controlClient, listed);
      egressProfiles = localPresets.profiles;
      if (localPresets.error) {
        egressProfileError = localPresets.error;
        showGlobalMessage('warning', 'Local egress presets incomplete', localPresets.error);
      }
      if (showFeedback && !localPresets.error) {
        const createdSuffix = localPresets.created > 0
          ? ` Created ${localPresets.created} local preset${localPresets.created === 1 ? '' : 's'}.`
          : '';
        showGlobalMessage(
          'success',
          'Egress profiles refreshed',
          `${egressProfiles.length} profile${egressProfiles.length === 1 ? '' : 's'} refreshed.${createdSuffix}`,
        );
      }
    } catch (error) {
      egressProfileError = errorMessage(error);
      showGlobalMessage('warning', 'Egress profiles unavailable', egressProfileError);
    } finally {
      egressProfilesLoading = false;
    }
  }
  async function createEgressProfile(command: CreateEgressProfileCommand): Promise<EgressProfileResource> {
    egressProfilesLoading = true;
    egressProfileError = null;
    try {
      const created = await controlClient.createEgressProfile(command);
      egressProfiles = [created, ...egressProfiles.filter((profile) => profile.id !== created.id)];
      showGlobalMessage('success', 'Egress profile created', `Created egress profile ${created.name}.`);
      return created;
    } catch (error) {
      egressProfileError = errorMessage(error);
      showGlobalMessage('error', 'Egress profile create failed', egressProfileError);
      throw error;
    } finally {
      egressProfilesLoading = false;
    }
  }
  async function updateEgressProfile(profileId: string, command: CreateEgressProfileCommand): Promise<EgressProfileResource> {
    egressProfilesLoading = true;
    egressProfileError = null;
    try {
      const updated = await controlClient.updateEgressProfile(profileId, command);
      egressProfiles = egressProfiles.some((profile) => profile.id === updated.id)
        ? egressProfiles.map((profile) => profile.id === updated.id ? updated : profile)
        : [updated, ...egressProfiles];
      showGlobalMessage('success', 'Egress profile updated', `Updated egress profile ${updated.name}.`);
      return updated;
    } catch (error) {
      egressProfileError = errorMessage(error);
      showGlobalMessage('error', 'Egress profile update failed', egressProfileError);
      throw error;
    } finally {
      egressProfilesLoading = false;
    }
  }
  async function runEgressProfileReachabilityProbe(profileId: string): Promise<EgressDiagnosticsResource> {
    egressProfilesLoading = true;
    egressProfileError = null;
    try {
      const diagnostics = await controlClient.runEgressProfileReachabilityProbe(profileId);
      const refreshed = await controlClient.getEgressProfile(profileId);
      egressProfiles = egressProfiles.some((profile) => profile.id === refreshed.id)
        ? egressProfiles.map((profile) => profile.id === refreshed.id ? refreshed : profile)
        : [refreshed, ...egressProfiles];
      showGlobalMessage(
        diagnostics.proof.profile_reachability_healthy ? 'success' : 'warning',
        diagnostics.proof.profile_reachability_healthy ? 'Egress profile reachable' : 'Egress profile reachability failed',
        diagnostics.proof.profile_reachability_healthy
          ? `${refreshed.name} can reach its configured egress endpoint.`
          : diagnostics.proof.profile_reachability_failure ?? 'The configured egress endpoint did not accept a gateway TCP connection.',
      );
      return diagnostics;
    } catch (error) {
      egressProfileError = errorMessage(error);
      showGlobalMessage('error', 'Egress profile reachability failed', egressProfileError);
      throw error;
    } finally {
      egressProfilesLoading = false;
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
      void loadProjects(false);
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
  async function cloneBrowserContext(contextId: string, command: CloneBrowserContextCommand): Promise<BrowserContextResource> {
    cloningBrowserContextId = contextId;
    browserContextError = null;
    try {
      const cloned = await controlClient.cloneBrowserContext(contextId, command);
      browserContexts = [cloned, ...browserContexts.filter((context) => context.id !== cloned.id)];
      showGlobalMessage('success', 'Browser context cloned', `Cloned context ${shortAdminId(contextId)} to ${shortAdminId(cloned.id)}.`);
      return cloned;
    } catch (error) {
      browserContextError = errorMessage(error);
      showGlobalMessage('error', 'Browser context clone failed', browserContextError);
      throw error;
    } finally {
      cloningBrowserContextId = null;
    }
  }
  async function exportBrowserContext(contextId: string): Promise<void> {
    exportingBrowserContextId = contextId;
    browserContextError = null;
    try {
      const context = browserContexts.find((candidate) => candidate.id === contextId);
      const blob = await controlClient.exportBrowserContext(contextId);
      saveBlob(blob, `browserpane-browser-context-${context?.name ?? contextId}.zip`);
      showGlobalMessage('success', 'Browser context export started', `Exported context ${shortAdminId(contextId)}.`);
    } catch (error) {
      browserContextError = errorMessage(error);
      showGlobalMessage('error', 'Browser context export failed', browserContextError);
      throw error;
    } finally {
      exportingBrowserContextId = null;
    }
  }
  async function importBrowserContext(command: ImportBrowserContextCommand): Promise<BrowserContextResource> {
    importingBrowserContext = true;
    browserContextError = null;
    try {
      const imported = await controlClient.importBrowserContext(command);
      browserContexts = [imported, ...browserContexts.filter((context) => context.id !== imported.id)];
      showGlobalMessage('success', 'Browser context imported', `Imported context ${shortAdminId(imported.id)}.`);
      return imported;
    } catch (error) {
      browserContextError = errorMessage(error);
      showGlobalMessage('error', 'Browser context import failed', browserContextError);
      throw error;
    } finally {
      importingBrowserContext = false;
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
  async function runSelectedSessionEgressProbe(): Promise<void> {
    if (!selectedSession) {
      showGlobalMessage('warning', 'Egress probe skipped', 'Select a session before running an egress probe.');
      return;
    }
    const sessionId = selectedSession.id;
    sessionsLoading = true;
    sessionsError = null;
    showGlobalMessage('loading', 'Egress probe', `Running browser egress probe for session ${shortAdminId(sessionId)}...`);
    try {
      const diagnostics = await controlClient.runSessionEgressDiagnosticsProbe(sessionId);
      upsertSession(await controlClient.getSession(sessionId));
      showGlobalMessage(
        diagnostics.proof.active_probe_collected ? 'success' : 'warning',
        diagnostics.proof.active_probe_collected ? 'Egress probe collected' : 'Egress probe failed',
        diagnostics.proof.active_probe_collected
          ? `Observed ${diagnostics.proof.observed_public_ip ?? 'egress'}${diagnostics.proof.observed_tls_issuer ? ` via ${diagnostics.proof.observed_tls_issuer}` : ''}.`
          : diagnostics.proof.last_failure_reason ?? 'The browser egress probe did not collect evidence.',
      );
    } catch (error) {
      sessionsError = errorMessage(error);
      showGlobalMessage('error', 'Egress probe failed', sessionsError);
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
      {projects} {projectsLoading} {projectError}
      {sessions} {browserContexts} {browserContextsLoading} {browserContextError}
      {egressProfiles} {egressProfilesLoading} {egressProfileError}
      cloningContextId={cloningBrowserContextId}
      exportingContextId={exportingBrowserContextId}
      {browserPreferences} {browserConnected} {workspaceViewModel} {sessionListViewModel} {logEntries} {globalMessage}
      {sessionFilesRefreshVersion} {recordingsRefreshVersion} {mcpDelegationRefreshVersion} onRefreshSessions={loadSessions}
      onCreateSession={(command) => void createSession(command)} onJoinSelectedSession={requestBrowserConnect}
      onRunSelectedSessionEgressProbe={runSelectedSessionEgressProbe}
      onCreateBrowserContext={createBrowserContext}
      onCreateEgressProfile={createEgressProfile}
      onUpdateEgressProfile={updateEgressProfile}
      onRunEgressProfileReachabilityProbe={runEgressProfileReachabilityProbe}
      onCloneBrowserContext={cloneBrowserContext}
      onExportBrowserContext={exportBrowserContext}
      onImportBrowserContext={importBrowserContext}
      {importingBrowserContext}
      onRefreshBrowserContexts={loadBrowserContexts}
      onRefreshEgressProfiles={loadEgressProfiles}
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
