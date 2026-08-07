<script lang="ts">
  import { ArrowLeft, RefreshCw } from '@lucide/svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import {
    AdminEventClient,
    type AdminEventConnectionStatus,
    type AdminEventSubscription,
  } from '$lib/api/admin-event-client';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { projectToneClass } from '$lib/projects/project-ui';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import {
    applySessionAdminEvent,
    buildSessionObservabilityModel,
    createSessionObservabilityEvidence,
    type SessionObservabilityEvidence,
  } from '$lib/sessions/session-observability-view-model';
  import type { SessionResource, SessionStatus } from '$lib/sessions/session-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import SessionSubareaNavigation from './SessionSubareaNavigation.svelte';

  type SessionObservabilityRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly sessionId: string;
  };

  type SessionObservabilityRouteState =
    | { readonly status: 'loading' }
    | { readonly status: 'error'; readonly message: string }
    | { readonly status: 'ready'; readonly evidence: SessionObservabilityEvidence };

  let { authContext, sessionId }: SessionObservabilityRouteProps = $props();
  let loadedSessionId = $state<string | null>(null);
  let routeState = $state<SessionObservabilityRouteState>({ status: 'loading' });
  let actionState = $state<AdminActionState>({ status: 'idle' });
  let streamStatus = $state<AdminEventConnectionStatus>('closed');
  let streamError = $state<string | null>(null);

  const model = $derived(routeState.status === 'ready'
    ? buildSessionObservabilityModel(routeState.evidence, streamStatus, streamError)
    : null);
  const busy = $derived(routeState.status === 'loading' || actionState.status === 'running');

  $effect(() => {
    const requestSessionId = sessionId;
    let disposed = false;
    let subscription: AdminEventSubscription | null = null;
    loadedSessionId = requestSessionId;
    routeState = { status: 'loading' };
    actionState = { status: 'idle' };
    streamStatus = 'connecting';
    streamError = null;

    void loadInitialEvidence(requestSessionId).then((evidence) => {
      if (disposed || loadedSessionId !== requestSessionId || !evidence) {
        return;
      }
      routeState = { status: 'ready', evidence };
      subscription = subscribe(requestSessionId, () => disposed);
    });

    return () => {
      disposed = true;
      subscription?.close();
    };
  });

  function clientOptions() {
    return {
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    };
  }

  async function loadInitialEvidence(requestSessionId: string): Promise<SessionObservabilityEvidence | null> {
    try {
      const client = new SessionCatalogClient(clientOptions());
      const session = await client.getSession(requestSessionId);
      const liveStatus = await loadStatus(client, requestSessionId);
      return createSessionObservabilityEvidence(session, liveStatus);
    } catch (error) {
      if (loadedSessionId === requestSessionId) {
        routeState = {
          status: 'error',
          message: adminErrorMessage(error, 'Unexpected session observability route error.'),
        };
      }
      return null;
    }
  }

  async function loadStatus(client: SessionCatalogClient, requestSessionId: string): Promise<SessionStatus | null> {
    try {
      return await client.getSessionStatus(requestSessionId);
    } catch {
      return null;
    }
  }

  function subscribe(
    requestSessionId: string,
    disposed: () => boolean,
  ): AdminEventSubscription {
    return new AdminEventClient(clientOptions()).subscribe({
      onEvent: (event) => {
        if (disposed() || loadedSessionId !== requestSessionId || routeState.status !== 'ready') {
          return;
        }
        routeState = {
          status: 'ready',
          evidence: applySessionAdminEvent(routeState.evidence, event, requestSessionId),
        };
      },
      onStatus: (status) => {
        if (disposed() || loadedSessionId !== requestSessionId) {
          return;
        }
        streamStatus = status;
        if (status === 'open') {
          streamError = null;
        }
      },
      onError: (error) => {
        if (!disposed() && loadedSessionId === requestSessionId) {
          streamError = error.message;
        }
      },
    });
  }

  async function refreshCurrentEvidence(): Promise<void> {
    const requestSessionId = sessionId;
    actionState = { status: 'running', label: 'Refreshing current session evidence...' };
    try {
      const client = new SessionCatalogClient(clientOptions());
      const session = await client.getSession(requestSessionId);
      const liveStatus = await loadStatus(client, requestSessionId);
      if (loadedSessionId !== requestSessionId || routeState.status !== 'ready') {
        return;
      }
      routeState = {
        status: 'ready',
        evidence: { ...routeState.evidence, session, liveStatus },
      };
      actionState = { status: 'success', message: 'Current session evidence refreshed.' };
    } catch (error) {
      if (loadedSessionId === requestSessionId) {
        actionState = {
          status: 'error',
          message: adminErrorMessage(error, 'Session evidence refresh failed.'),
        };
      }
    }
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="session-observability-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink" href="/admin-new/sessions" data-testid="session-observability-back">
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Sessions</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Session observability</h1>
      <p class="m-0 mt-1 min-w-0 break-all font-mono text-xs text-admin-muted">{sessionId}</p>
    </div>
    <button
      class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={() => void refreshCurrentEvidence()}
      disabled={busy}
      data-testid="session-observability-refresh"
    >
      <RefreshCw size={15} strokeWidth={1.8} />
      <span>Refresh current state</span>
    </button>
  </header>

  <SessionSubareaNavigation
    {sessionId}
    activeId="observability"
    availableIds={['overview', 'live', 'automation', 'policy', 'files', 'recordings', 'network', 'observability']}
  />

  <ActionFeedback
    state={actionState}
    successTitle="Session evidence refreshed"
    errorTitle="Session evidence refresh failed"
    successTestId="session-observability-action-success"
    errorTestId="session-observability-action-error"
    runningTestId="session-observability-action-running"
  />

  {#if routeState.status === 'loading'}
    <section class="flex min-h-64 items-center justify-center rounded-md border border-admin-border bg-admin-panel p-6 text-sm text-admin-muted" data-testid="session-observability-loading">
      Loading current session evidence...
    </section>
  {:else if routeState.status === 'error'}
    <AdminMessage tone="error" title="Session observability unavailable" message={routeState.message} testId="session-observability-error" />
  {:else if model}
    <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-observability-stream">
      <div class="flex min-w-0 flex-col gap-3 p-4 sm:flex-row sm:items-start sm:justify-between">
        <div class="min-w-0">
          <h2 class="m-0 text-base font-semibold text-admin-ink">Admin event stream</h2>
          <p class="m-0 mt-1 text-sm text-admin-muted" data-testid="session-observability-stream-description">{model.streamDescription}</p>
        </div>
        <span class={`inline-flex w-fit shrink-0 rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(model.streamTone)}`} data-testid="session-observability-stream-status">
          {model.streamLabel}
        </span>
      </div>
    </section>

    <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-observability-current">
      <div class="border-b border-admin-border p-4">
        <h2 class="m-0 text-base font-semibold text-admin-ink">Current state</h2>
        <p class="m-0 mt-1 text-sm text-admin-muted">Live resource facts; explicit refresh supplements stream snapshots where needed.</p>
      </div>
      <dl class="grid gap-px bg-admin-border sm:grid-cols-2 lg:grid-cols-5">
        {#each model.facts as fact}
          <div class="min-w-0 bg-admin-panel p-4">
            <dt class="text-xs font-semibold uppercase text-admin-muted">{fact.label}</dt>
            <dd class="m-0 mt-2">
              <span class={`inline-flex max-w-full rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(fact.tone)}`} data-testid={fact.testId}>
                <span class="min-w-0 break-words">{fact.value}</span>
              </span>
            </dd>
          </div>
        {/each}
      </dl>
    </section>

    <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-observability-timeline">
      <div class="border-b border-admin-border p-4">
        <h2 class="m-0 text-base font-semibold text-admin-ink">Local snapshot timeline</h2>
        <p class="m-0 mt-1 text-sm text-admin-muted">Bounded to 40 entries in this browser view. This is operational context, not a durable audit log.</p>
      </div>
      {#if model.timeline.length === 0}
        <p class="m-4 rounded-md border border-dashed border-admin-border bg-admin-soft/50 p-4 text-sm text-admin-muted" data-testid="session-observability-timeline-empty">
          Waiting for the first session-scoped snapshot.
        </p>
      {:else}
        <ol class="m-0 divide-y divide-admin-border p-0" data-testid="session-observability-timeline-list">
          {#each model.timeline as entry}
            <li class="flex min-w-0 flex-col gap-2 p-4 sm:flex-row sm:items-start sm:justify-between" data-testid="session-observability-event" data-event-id={entry.id}>
              <div class="min-w-0">
                <div class="flex min-w-0 flex-wrap items-center gap-2">
                  <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(entry.tone)}`}>{entry.source}</span>
                  <strong class="text-sm font-semibold text-admin-ink">{entry.title}</strong>
                </div>
                <p class="m-0 mt-1 text-sm text-admin-muted">{entry.detail}</p>
              </div>
              <time class="shrink-0 text-xs text-admin-muted" datetime={entry.createdAt}>{entry.createdAt}</time>
            </li>
          {/each}
        </ol>
      {/if}
    </section>
  {/if}
</div>
