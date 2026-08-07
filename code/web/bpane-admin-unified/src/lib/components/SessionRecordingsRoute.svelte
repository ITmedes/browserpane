<script lang="ts">
  import { ArrowLeft, Download, RefreshCw, Video, VideoOff } from '@lucide/svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { projectToneClass } from '$lib/projects/project-ui';
  import { RecordingCatalogClient } from '$lib/recordings/recording-client';
  import {
    buildSessionRecordingDetailModel,
    type SessionRecordingDetailModel,
  } from '$lib/recordings/session-recording-detail-view-model';
  import type {
    SessionRecordingPlaybackResource,
    SessionRecordingResource,
  } from '$lib/recordings/recording-types';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import type { SessionResource } from '$lib/sessions/session-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import SessionSubareaNavigation from './SessionSubareaNavigation.svelte';

  type SessionRecordingsRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly sessionId: string;
  };

  type SessionRecordingsRouteState =
    | { readonly status: 'loading' }
    | { readonly status: 'error'; readonly message: string }
    | {
        readonly status: 'ready';
        readonly session: SessionResource;
        readonly recordings: readonly SessionRecordingResource[];
        readonly recordingsError: string | null;
        readonly playback: SessionRecordingPlaybackResource | null;
        readonly playbackError: string | null;
      };

  let { authContext, sessionId }: SessionRecordingsRouteProps = $props();
  let loadedSessionId = $state<string | null>(null);
  let routeState = $state<SessionRecordingsRouteState>({ status: 'loading' });
  let actionState = $state<AdminActionState>({ status: 'idle' });

  const sessionClient = $derived(new SessionCatalogClient(clientOptions()));
  const recordingClient = $derived(new RecordingCatalogClient(clientOptions()));
  const model = $derived<SessionRecordingDetailModel | null>(routeState.status === 'ready'
    ? buildSessionRecordingDetailModel(routeState.session, routeState.recordings, routeState.playback)
    : null);
  const busy = $derived(routeState.status === 'loading' || actionState.status === 'running');

  $effect(() => {
    if (sessionId === loadedSessionId) {
      return;
    }
    loadedSessionId = sessionId;
    routeState = { status: 'loading' };
    actionState = { status: 'idle' };
    void loadRoute(sessionId, false);
  });

  function clientOptions() {
    return {
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    };
  }

  async function loadRoute(requestSessionId = sessionId, showFeedback = true): Promise<void> {
    if (showFeedback) {
      actionState = { status: 'running', label: 'Refreshing session recordings...' };
    }
    try {
      const session = await sessionClient.getSession(requestSessionId);
      const [recordingsResult, playbackResult] = await Promise.allSettled([
        recordingClient.listSessionRecordings(requestSessionId),
        recordingClient.getSessionRecordingPlayback(requestSessionId),
      ]);
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      routeState = {
        status: 'ready',
        session,
        recordings: recordingsResult.status === 'fulfilled' ? recordingsResult.value.recordings : [],
        recordingsError: recordingsResult.status === 'rejected'
          ? adminErrorMessage(recordingsResult.reason, 'Session recording segments are unavailable.')
          : null,
        playback: playbackResult.status === 'fulfilled' ? playbackResult.value : null,
        playbackError: playbackResult.status === 'rejected'
          ? adminErrorMessage(playbackResult.reason, 'Session playback summary is unavailable.')
          : null,
      };
      if (showFeedback) {
        actionState = recordingsResult.status === 'fulfilled' && playbackResult.status === 'fulfilled'
          ? { status: 'success', message: 'Session recordings refreshed.' }
          : { status: 'error', message: 'Session recordings refreshed with partial results.' };
      }
    } catch (error) {
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      const message = adminErrorMessage(error, 'Unexpected session recordings route error.');
      routeState = { status: 'error', message };
      if (showFeedback) {
        actionState = { status: 'error', message };
      }
    }
  }

  async function setRecordingPolicy(enabled: boolean): Promise<void> {
    if (routeState.status !== 'ready') {
      return;
    }
    const requestSessionId = routeState.session.id;
    actionState = {
      status: 'running',
      label: enabled ? 'Enabling always-on recording...' : 'Disabling always-on recording...',
    };
    try {
      const session = await sessionClient.updateSessionRecordingPolicy(requestSessionId, {
        mode: enabled ? 'always' : 'disabled',
        format: 'webm',
        ...(routeState.session.recording.retention_sec
          ? { retention_sec: routeState.session.recording.retention_sec }
          : {}),
      });
      if (loadedSessionId !== requestSessionId || routeState.status !== 'ready') {
        return;
      }
      routeState = { ...routeState, session };
      actionState = {
        status: 'success',
        message: enabled
          ? session.state === 'stopped' || session.state === 'released' || session.state === 'queued'
            ? 'Always-on recording enabled. Recording starts with the next session runtime.'
            : 'Always-on recording enabled.'
          : 'Always-on recording disabled.',
      };
    } catch (error) {
      if (loadedSessionId === requestSessionId) {
        actionState = {
          status: 'error',
          message: adminErrorMessage(error, 'Session recording policy update failed.'),
        };
      }
    }
  }

  async function downloadPlayback(): Promise<void> {
    if (!model?.canDownload || routeState.status !== 'ready') {
      return;
    }
    const requestSessionId = routeState.session.id;
    actionState = { status: 'running', label: `${model.downloadLabel}...` };
    try {
      const blob = model.downloadKind === 'segment' && model.downloadableRecording
        ? await recordingClient.downloadRecordingContent(model.downloadableRecording)
        : await recordingClient.downloadSessionPlaybackExport(requestSessionId);
      triggerDownload(blob, model.downloadFileName);
      if (loadedSessionId === requestSessionId) {
        actionState = { status: 'success', message: 'Recording download started.' };
      }
    } catch (error) {
      if (loadedSessionId === requestSessionId) {
        actionState = {
          status: 'error',
          message: adminErrorMessage(error, 'Recording download failed.'),
        };
      }
    }
  }

  function triggerDownload(blob: Blob, fileName: string): void {
    const url = URL.createObjectURL(blob);
    try {
      const link = document.createElement('a');
      link.href = url;
      link.download = fileName;
      document.body.append(link);
      link.click();
      link.remove();
    } finally {
      URL.revokeObjectURL(url);
    }
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="session-recordings-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink" href="/admin-new/sessions" data-testid="session-recordings-back">
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Sessions</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Session recordings</h1>
      <p class="m-0 mt-1 min-w-0 break-all font-mono text-xs text-admin-muted">{sessionId}</p>
    </div>
    <button
      class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={() => void loadRoute()}
      disabled={busy}
      data-testid="session-recordings-refresh"
    >
      <RefreshCw size={15} strokeWidth={1.8} />
      <span>Refresh</span>
    </button>
  </header>

  <SessionSubareaNavigation
    sessionId={sessionId}
    activeId="recordings"
    availableIds={['overview', 'live', 'files', 'recordings', 'network']}
  />

  <ActionFeedback
    state={actionState}
    successTitle="Recording action completed"
    errorTitle="Recording action failed"
    successTestId="session-recordings-action-success"
    errorTestId="session-recordings-action-error"
    runningTestId="session-recordings-action-running"
  />

  {#if routeState.status === 'loading'}
    <section class="flex min-h-64 items-center justify-center rounded-md border border-admin-border bg-admin-panel p-6 text-sm text-admin-muted" data-testid="session-recordings-loading">
      Loading session recordings...
    </section>
  {:else if routeState.status === 'error'}
    <AdminMessage tone="error" title="Session recordings unavailable" message={routeState.message} testId="session-recordings-error" />
  {:else if model}
    <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-recording-policy">
      <div class="flex flex-col gap-4 p-4 sm:flex-row sm:items-start sm:justify-between">
        <div class="min-w-0">
          <h2 class="m-0 text-base font-semibold text-admin-ink">Recording policy</h2>
          <p class="m-0 mt-1 text-sm text-admin-muted" data-testid="session-recording-policy-value">{model.policyLabel}</p>
        </div>
        <div class="flex shrink-0 flex-wrap gap-2">
          {#if model.policyEnabled}
            <button
              class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
              type="button"
              onclick={() => void setRecordingPolicy(false)}
              disabled={busy || !model.canDisablePolicy}
              data-testid="session-recording-disable"
            >
              <VideoOff size={15} strokeWidth={1.8} />
              <span>Disable</span>
            </button>
          {:else}
            <button
              class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
              type="button"
              onclick={() => void setRecordingPolicy(true)}
              disabled={busy || !model.canEnablePolicy}
              data-testid="session-recording-enable"
            >
              <Video size={15} strokeWidth={1.8} />
              <span>Enable always-on</span>
            </button>
          {/if}
        </div>
      </div>
    </section>

    <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-recording-playback">
      <div class="flex flex-col gap-4 border-b border-admin-border p-4 sm:flex-row sm:items-start sm:justify-between">
        <div class="min-w-0">
          <div class="flex flex-wrap items-center gap-2">
            <h2 class="m-0 text-base font-semibold text-admin-ink">Playback</h2>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(model.playbackTone)}`} data-testid="session-recording-playback-state">
              {model.playbackState}
            </span>
          </div>
          <p class="m-0 mt-1 text-xs text-admin-muted" data-testid="session-recording-playback-summary">{model.playbackSummary}</p>
        </div>
        <button
          class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
          type="button"
          onclick={() => void downloadPlayback()}
          disabled={busy || !model.canDownload}
          data-testid="session-recording-download"
        >
          <Download size={15} strokeWidth={1.8} />
          <span>{model.downloadLabel}</span>
        </button>
      </div>
      <div class="grid gap-3 p-4 sm:grid-cols-2">
        {#if routeState.playbackError}
          <div class="sm:col-span-2">
            <AdminMessage tone="warning" density="compact" title="Playback summary unavailable" message={routeState.playbackError} testId="session-recording-playback-error" />
          </div>
        {/if}
        <div class="rounded-md border border-admin-border bg-admin-soft/50 p-3">
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Included size</p>
          <p class="m-0 mt-1 text-sm font-medium text-admin-ink" data-testid="session-recording-playback-size">{model.includedSize}</p>
        </div>
        <div class="rounded-md border border-admin-border bg-admin-soft/50 p-3">
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Included duration</p>
          <p class="m-0 mt-1 text-sm font-medium text-admin-ink" data-testid="session-recording-playback-duration">{model.includedDuration}</p>
        </div>
      </div>
    </section>

    <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-recording-segments">
      <div class="border-b border-admin-border p-4">
        <h2 class="m-0 text-base font-semibold text-admin-ink">Segments</h2>
        <p class="m-0 mt-1 text-xs text-admin-muted">{model.rows.length} retained segment{model.rows.length === 1 ? '' : 's'}</p>
      </div>
      <div class="grid gap-3 p-4">
        {#if routeState.recordingsError}
          <AdminMessage tone="error" density="compact" title="Recording segments unavailable" message={routeState.recordingsError} testId="session-recording-segments-error" />
        {:else if model.rows.length === 0}
          <div class="rounded-md border border-dashed border-admin-border bg-admin-soft/50 p-5 text-center text-sm text-admin-muted" data-testid="session-recording-segments-empty">
            No recording segments are retained for this session.
          </div>
        {:else}
          {#each model.rows as row}
            <article class="grid min-w-0 gap-3 rounded-md border border-admin-border bg-admin-soft/50 p-3 lg:grid-cols-[minmax(0,1fr)_minmax(220px,auto)]" data-testid="session-recording-segment-row">
              <div class="min-w-0">
                <div class="flex min-w-0 flex-wrap items-center gap-2">
                  <strong class="break-all font-mono text-sm font-semibold text-admin-ink">{row.id}</strong>
                  <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.stateTone)}`}>{row.state}</span>
                </div>
                <p class="m-0 mt-1 text-xs text-admin-muted">{row.artifact} | {row.format} | {row.size} | {row.duration}</p>
                {#if row.previousId}
                  <p class="m-0 mt-1 break-all text-xs text-admin-muted">Previous: <span class="font-mono">{row.previousId}</span></p>
                {/if}
                {#if row.error}
                  <p class="m-0 mt-2 text-sm font-medium text-red-700" data-testid="session-recording-segment-error">{row.error}</p>
                {/if}
              </div>
              <dl class="m-0 grid content-start gap-1 text-xs text-admin-muted">
                <div class="flex justify-between gap-4"><dt>Started</dt><dd class="m-0 text-right text-admin-ink">{row.startedAt}</dd></div>
                <div class="flex justify-between gap-4"><dt>Completed</dt><dd class="m-0 text-right text-admin-ink">{row.completedAt}</dd></div>
                <div class="flex justify-between gap-4"><dt>Termination</dt><dd class="m-0 text-right text-admin-ink">{row.termination}</dd></div>
              </dl>
            </article>
          {/each}
        {/if}
      </div>
    </section>
  {/if}
</div>
