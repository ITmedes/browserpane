<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import { ControlApiError, type ControlClient } from '../api/control-client';
  import type {
    SessionFileBindingResource,
    SessionFileResource,
    SessionResource,
  } from '../api/control-types';
  import type { SessionRecordingPlaybackResource, SessionRecordingResource } from '../api/recording-types';
  import type { SessionStatus } from '../api/session-status-types';
  import SessionDetailPanel from '../presentation/SessionDetailPanel.svelte';
  import { SessionViewModelBuilder } from '../presentation/session-view-model';
  import SessionFileBindingsSurface from './SessionFileBindingsSurface.svelte';

  type AdminSessionDetailRouteProps = {
    readonly controlClient: ControlClient;
    readonly sessionId: string;
  };

  let { controlClient, sessionId }: AdminSessionDetailRouteProps = $props();
  let session = $state<SessionResource | null>(null);
  let status = $state<SessionStatus | null>(null);
  let files = $state<readonly SessionFileResource[]>([]);
  let bindings = $state<readonly SessionFileBindingResource[]>([]);
  let recordings = $state<readonly SessionRecordingResource[]>([]);
  let playback = $state<SessionRecordingPlaybackResource | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let relatedError = $state<string | null>(null);
  let lastRefreshedAt = $state<string | null>(null);

  const viewModel = $derived(SessionViewModelBuilder.detail({
    session,
    status,
    connected: false,
    loading,
    error,
  }));

  onMount(() => {
    void refreshInspector();
  });

  async function refreshInspector(): Promise<void> {
    loading = true;
    error = null;
    relatedError = null;
    try {
      const [nextSession, nextStatus] = await Promise.all([
        controlClient.getSession(sessionId),
        controlClient.getSessionStatus(sessionId),
      ]);
      session = nextSession;
      status = nextStatus;
      await loadRelatedResources();
      lastRefreshedAt = new Date().toISOString();
    } catch (refreshError) {
      error = errorMessage(refreshError, 'Unexpected session detail error');
    } finally {
      loading = false;
    }
  }

  async function loadRelatedResources(): Promise<void> {
    const [fileResult, bindingResult, recordingResult, playbackResult] = await Promise.allSettled([
      controlClient.listSessionFiles(sessionId),
      controlClient.listSessionFileBindings(sessionId),
      controlClient.listSessionRecordings(sessionId),
      controlClient.getSessionRecordingPlayback(sessionId),
    ]);
    const errors: string[] = [];
    if (fileResult.status === 'fulfilled') {
      files = fileResult.value.files;
    } else {
      files = [];
      errors.push(errorMessage(fileResult.reason, 'Session file summary failed'));
    }
    if (bindingResult.status === 'fulfilled') {
      bindings = bindingResult.value.bindings;
    } else {
      bindings = [];
      errors.push(errorMessage(bindingResult.reason, 'Session file binding summary failed'));
    }
    if (recordingResult.status === 'fulfilled') {
      recordings = recordingResult.value.recordings;
    } else {
      recordings = [];
      errors.push(errorMessage(recordingResult.reason, 'Recording summary failed'));
    }
    if (playbackResult.status === 'fulfilled') {
      playback = playbackResult.value;
    } else {
      playback = null;
      errors.push(errorMessage(playbackResult.reason, 'Recording playback summary failed'));
    }
    relatedError = errors.length > 0 ? errors.join(' | ') : null;
  }

  async function stopSession(): Promise<void> {
    await mutateSession(() => controlClient.stopSession(sessionId));
  }

  async function killSession(): Promise<void> {
    await mutateSession(() => controlClient.killSession(sessionId));
  }

  async function disconnectConnection(connectionId: number): Promise<void> {
    await mutateStatus(() => controlClient.disconnectSessionConnection(sessionId, connectionId));
  }

  async function disconnectAllConnections(): Promise<void> {
    await mutateStatus(() => controlClient.disconnectAllSessionConnections(sessionId));
  }

  async function mutateSession(action: () => Promise<SessionResource>): Promise<void> {
    loading = true;
    error = null;
    try {
      session = await action();
      status = await controlClient.getSessionStatus(sessionId);
      await loadRelatedResources();
      lastRefreshedAt = new Date().toISOString();
    } catch (mutationError) {
      error = errorMessage(mutationError, 'Unexpected session action error');
    } finally {
      loading = false;
    }
  }

  async function mutateStatus(action: () => Promise<SessionStatus>): Promise<void> {
    loading = true;
    error = null;
    try {
      status = await action();
      session = await controlClient.getSession(sessionId);
      lastRefreshedAt = new Date().toISOString();
    } catch (mutationError) {
      error = errorMessage(mutationError, 'Unexpected connection action error');
    } finally {
      loading = false;
    }
  }

  function errorMessage(value: unknown, fallback: string): string {
    if (value instanceof ControlApiError && value.status === 404) {
      return `Session ${sessionId} was not found.`;
    }
    return value instanceof Error ? value.message : fallback;
  }

  function formatDate(value: string | null): string {
    if (!value) {
      return 'not refreshed';
    }
    return new Date(value).toLocaleString();
  }

  function byteLabel(value: number): string {
    if (value < 1024) {
      return `${value} B`;
    }
    if (value < 1024 * 1024) {
      return `${(value / 1024).toFixed(1)} KiB`;
    }
    return `${(value / 1024 / 1024).toFixed(1)} MiB`;
  }

  const readyRecordingCount = $derived(recordings.filter((entry) => entry.state === 'ready').length);
  const activeRecordingCount = $derived(recordings.filter((entry) => entry.state === 'recording').length);
  const fileBytes = $derived(files.reduce((total, file) => total + file.byte_count, 0));
  const bindingBytes = $derived(bindings.reduce((total, binding) => total + binding.byte_count, 0));
</script>

<section class="grid gap-5" data-testid="session-inspector-detail">
  <div class="admin-panel mt-0">
    <div class="admin-header">
      <div class="min-w-0">
        <p class="admin-eyebrow">Session detail</p>
        <h1 class="m-0 truncate font-mono text-xl font-bold text-admin-ink" data-testid="session-inspector-title">
          {session?.id ?? sessionId}
        </h1>
      </div>
      <div class="admin-actions">
        <a class="admin-button-ghost" href={`${base}/sessions`}>Sessions</a>
        <a class="admin-button-ghost" href={`${base}/files/workspaces`}>File workspaces</a>
        <a class="admin-button-ghost" href={`${base}/`}>Live workspace</a>
        <button
          class="admin-button-primary"
          type="button"
          data-testid="session-inspector-detail-refresh"
          disabled={loading}
          onclick={() => void refreshInspector()}
        >
          Refresh
        </button>
      </div>
    </div>
    <p class="m-0 mt-3 text-sm text-admin-ink/62" data-testid="session-inspector-last-refresh">
      Last refreshed {formatDate(lastRefreshedAt)}
    </p>
  </div>

  {#if loading && !session}
    <section class="admin-panel mt-0">
      <p class="admin-empty mt-0">Loading session...</p>
    </section>
  {:else if error && !session}
    <section class="admin-panel mt-0">
      <p class="admin-error mt-0" data-testid="session-inspector-detail-error">{error}</p>
    </section>
  {:else}
    <section class="admin-panel mt-0">
      <SessionDetailPanel
        {viewModel}
        onRefresh={() => void refreshInspector()}
        onStop={() => void stopSession()}
        onKill={() => void killSession()}
        onDisconnectConnection={(connectionId) => void disconnectConnection(connectionId)}
        onDisconnectAll={() => void disconnectAllConnections()}
      />
    </section>

    <section class="grid gap-3 md:grid-cols-4" aria-label="Related session resources">
      <article class="rounded-2xl border border-admin-ink/10 bg-admin-panel/82 p-4">
        <p class="admin-eyebrow">Runtime files</p>
        <strong class="block text-2xl text-admin-ink" data-testid="session-inspector-files-count">
          {files.length}
        </strong>
        <p class="m-0 mt-2 text-sm text-admin-ink/62">{byteLabel(fileBytes)} retained runtime data</p>
      </article>

      <article class="rounded-2xl border border-admin-ink/10 bg-admin-panel/82 p-4">
        <p class="admin-eyebrow">Input bindings</p>
        <strong class="block text-2xl text-admin-ink" data-testid="session-inspector-binding-count">
          {bindings.length}
        </strong>
        <p class="m-0 mt-2 text-sm text-admin-ink/62">{byteLabel(bindingBytes)} mounted workspace inputs</p>
      </article>

      <article class="rounded-2xl border border-admin-ink/10 bg-admin-panel/82 p-4">
        <p class="admin-eyebrow">Recordings</p>
        <strong class="block text-2xl text-admin-ink" data-testid="session-inspector-recording-count">
          {recordings.length}
        </strong>
        <p class="m-0 mt-2 text-sm text-admin-ink/62">
          {readyRecordingCount} ready | {activeRecordingCount} active
        </p>
      </article>

      <article class="rounded-2xl border border-admin-ink/10 bg-admin-panel/82 p-4">
        <p class="admin-eyebrow">Playback</p>
        <strong class="block text-2xl text-admin-ink" data-testid="session-inspector-playback-count">
          {playback?.included_segment_count ?? status?.playback.included_segment_count ?? 0}
        </strong>
        <p class="m-0 mt-2 text-sm text-admin-ink/62">
          of {playback?.segment_count ?? status?.playback.segment_count ?? 0} retained segments
        </p>
      </article>
    </section>

    {#if relatedError}
      <p class="admin-error" data-testid="session-inspector-related-error">{relatedError}</p>
    {/if}

    <SessionFileBindingsSurface
      {controlClient}
      {sessionId}
      onBindingCountChange={(count) => {
        if (count === bindings.length) {
          return;
        }
        void loadRelatedResources();
      }}
    />
  {/if}
</section>
