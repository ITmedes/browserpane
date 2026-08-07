<script lang="ts">
  import { ArrowLeft, ExternalLink, RefreshCw, ScanSearch } from '@lucide/svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import type { EgressDiagnosticsResource } from '$lib/egress-profiles/egress-profile-types';
  import { projectToneClass } from '$lib/projects/project-ui';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import { buildSessionNetworkModel } from '$lib/sessions/session-network-view-model';
  import type { SessionResource } from '$lib/sessions/session-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import SessionSubareaNavigation from './SessionSubareaNavigation.svelte';

  type SessionNetworkRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly sessionId: string;
  };

  type SessionNetworkRouteState =
    | { readonly status: 'loading' }
    | { readonly status: 'error'; readonly message: string }
    | {
        readonly status: 'ready';
        readonly session: SessionResource;
        readonly diagnostics: EgressDiagnosticsResource | null;
        readonly diagnosticsError: string | null;
      };

  let { authContext, sessionId }: SessionNetworkRouteProps = $props();
  let loadedSessionId = $state<string | null>(null);
  let routeState = $state<SessionNetworkRouteState>({ status: 'loading' });
  let actionState = $state<AdminActionState>({ status: 'idle' });

  const client = $derived(new SessionCatalogClient({
    baseUrl: window.location.origin,
    accessTokenProvider: authContext.accessTokenProvider,
    onAuthenticationFailure: authContext.onAuthenticationFailure,
  }));
  const model = $derived(routeState.status === 'ready'
    ? buildSessionNetworkModel(routeState.session, routeState.diagnostics)
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

  async function loadRoute(requestSessionId = sessionId, showFeedback = true): Promise<void> {
    if (showFeedback) {
      actionState = { status: 'running', label: 'Refreshing session network evidence...' };
    }
    try {
      const session = await client.getSession(requestSessionId);
      const diagnosticsResult = await loadDiagnostics(requestSessionId);
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      routeState = {
        status: 'ready',
        session,
        diagnostics: diagnosticsResult.diagnostics ?? session.egress_diagnostics ?? null,
        diagnosticsError: diagnosticsResult.error,
      };
      if (showFeedback) {
        actionState = diagnosticsResult.error === null
          ? { status: 'success', message: 'Session network evidence refreshed.' }
          : { status: 'error', message: 'Session identity refreshed, but diagnostics remain unavailable.' };
      }
    } catch (error) {
      if (loadedSessionId !== requestSessionId) {
        return;
      }
      const message = adminErrorMessage(error, 'Unexpected session network route error.');
      routeState = { status: 'error', message };
      if (showFeedback) {
        actionState = { status: 'error', message };
      }
    }
  }

  async function loadDiagnostics(requestSessionId: string): Promise<{
    readonly diagnostics: EgressDiagnosticsResource | null;
    readonly error: string | null;
  }> {
    try {
      return {
        diagnostics: await client.getSessionEgressDiagnostics(requestSessionId),
        error: null,
      };
    } catch (error) {
      return {
        diagnostics: null,
        error: adminErrorMessage(error, 'Session egress diagnostics are unavailable.'),
      };
    }
  }

  async function runProbe(): Promise<void> {
    if (routeState.status !== 'ready' || !model?.canProbe) {
      return;
    }
    const requestSessionId = routeState.session.id;
    actionState = { status: 'running', label: 'Running active browser egress probe...' };
    try {
      const diagnostics = await client.runSessionEgressDiagnosticsProbe(requestSessionId);
      if (loadedSessionId !== requestSessionId || routeState.status !== 'ready') {
        return;
      }
      routeState = { ...routeState, diagnostics, diagnosticsError: null };
      actionState = diagnostics.proof.active_probe_collected
        ? { status: 'success', message: 'Active browser egress evidence collected.' }
        : {
            status: 'error',
            message: diagnostics.proof.last_failure_reason ?? 'The active browser egress probe did not collect evidence.',
          };
    } catch (error) {
      if (loadedSessionId === requestSessionId) {
        actionState = {
          status: 'error',
          message: adminErrorMessage(error, 'Active browser egress probe failed.'),
        };
      }
    }
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="session-network-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink" href="/admin-new/sessions" data-testid="session-network-back">
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Sessions</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Session network</h1>
      <p class="m-0 mt-1 min-w-0 break-all font-mono text-xs text-admin-muted">{sessionId}</p>
    </div>
    <button
      class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={() => void loadRoute()}
      disabled={busy}
      data-testid="session-network-refresh"
    >
      <RefreshCw size={15} strokeWidth={1.8} />
      <span>Refresh</span>
    </button>
  </header>

  <SessionSubareaNavigation
    sessionId={sessionId}
    activeId="network"
    availableIds={['overview', 'live', 'automation', 'policy', 'files', 'recordings', 'network']}
  />

  <ActionFeedback
    state={actionState}
    successTitle="Network action completed"
    errorTitle="Network action failed"
    successTestId="session-network-action-success"
    errorTestId="session-network-action-error"
    runningTestId="session-network-action-running"
  />

  {#if routeState.status === 'loading'}
    <section class="flex min-h-64 items-center justify-center rounded-md border border-admin-border bg-admin-panel p-6 text-sm text-admin-muted" data-testid="session-network-loading">
      Loading session network evidence...
    </section>
  {:else if routeState.status === 'error'}
    <AdminMessage tone="error" title="Session network unavailable" message={routeState.message} testId="session-network-error" />
  {:else if model}
    <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-network-summary">
      <div class="flex flex-col gap-4 p-4 md:flex-row md:items-start md:justify-between">
        <div class="min-w-0">
          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <h2 class="m-0 text-base font-semibold text-admin-ink">{model.modeLabel}</h2>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(model.healthTone)}`} data-testid="session-network-health">
              {model.health}
            </span>
            <span class="inline-flex rounded-full bg-slate-100 px-2 py-0.5 text-xs font-semibold text-slate-600 ring-1 ring-slate-200" data-testid="session-network-proof-level">
              {model.proofLevel}
            </span>
          </div>
          <div class="mt-2 flex min-w-0 flex-wrap items-center gap-2 text-sm text-admin-muted">
            <span class="break-all" data-testid="session-network-profile-label">{model.profileLabel}</span>
            {#if model.profileHref}
              <a class="inline-flex items-center gap-1 font-medium text-admin-accent hover:underline" href={model.profileHref} data-testid="session-network-profile-link">
                <span>Open profile</span>
                <ExternalLink size={13} strokeWidth={1.8} />
              </a>
            {/if}
          </div>
          {#if model.probeBlockedReason}
            <p class="m-0 mt-2 text-xs text-admin-muted" data-testid="session-network-probe-blocked">{model.probeBlockedReason}</p>
          {/if}
        </div>
        <button
          class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
          type="button"
          onclick={() => void runProbe()}
          disabled={busy || !model.canProbe}
          title={model.probeBlockedReason ?? 'Run active browser egress probe'}
          data-testid="session-network-probe"
        >
          <ScanSearch size={15} strokeWidth={1.8} />
          <span>Run active probe</span>
        </button>
      </div>
    </section>

    {#if routeState.diagnosticsError}
      <AdminMessage tone="warning" title="Diagnostics unavailable" message={routeState.diagnosticsError} testId="session-network-diagnostics-error" />
    {/if}

    <div class="grid min-w-0 gap-5 lg:grid-cols-2">
      <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-network-requested">
        <div class="border-b border-admin-border p-4">
          <h2 class="m-0 text-base font-semibold text-admin-ink">Requested identity</h2>
        </div>
        <dl class="m-0 divide-y divide-admin-border px-4">
          {#each model.requestedFacts as fact}
            <div class="grid min-w-0 gap-1 py-3 sm:grid-cols-[140px_minmax(0,1fr)] sm:gap-4">
              <dt class="text-xs font-semibold uppercase text-admin-muted">{fact.label}</dt>
              <dd class="m-0 min-w-0 break-all text-sm text-admin-ink" data-testid={fact.testId}>{fact.value}</dd>
            </div>
          {/each}
        </dl>
      </section>

      <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-network-effective">
        <div class="border-b border-admin-border p-4">
          <h2 class="m-0 text-base font-semibold text-admin-ink">Effective egress</h2>
        </div>
        <dl class="m-0 divide-y divide-admin-border px-4">
          {#each model.effectiveFacts as fact}
            <div class="grid min-w-0 gap-1 py-3 sm:grid-cols-[140px_minmax(0,1fr)] sm:gap-4">
              <dt class="text-xs font-semibold uppercase text-admin-muted">{fact.label}</dt>
              <dd class="m-0 min-w-0 break-all text-sm text-admin-ink" data-testid={fact.testId}>
                {#if fact.tone}
                  <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(fact.tone)}`}>{fact.value}</span>
                {:else}
                  {fact.value}
                {/if}
              </dd>
            </div>
          {/each}
        </dl>
      </section>
    </div>

    <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-network-proof">
      <div class="border-b border-admin-border p-4">
        <h2 class="m-0 text-base font-semibold text-admin-ink">Diagnostics proof</h2>
      </div>
      {#if model.proofFacts.length === 0}
        <div class="p-4">
          <AdminMessage tone="warning" density="compact" title="No diagnostics proof" message="No diagnostics resource is currently available for this session." testId="session-network-proof-empty" />
        </div>
      {:else}
        <dl class="m-0 grid min-w-0 gap-x-6 px-4 sm:grid-cols-2">
          {#each model.proofFacts as fact}
            <div class="grid min-w-0 gap-1 border-b border-admin-border py-3 sm:grid-cols-[140px_minmax(0,1fr)] sm:gap-4">
              <dt class="text-xs font-semibold uppercase text-admin-muted">{fact.label}</dt>
              <dd class="m-0 min-w-0 break-all text-sm text-admin-ink" data-testid={fact.testId}>
                {#if fact.tone}
                  <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(fact.tone)}`}>{fact.value}</span>
                {:else}
                  {fact.value}
                {/if}
              </dd>
            </div>
          {/each}
        </dl>
      {/if}
      {#if model.warnings.length > 0}
        <div class="p-4 pt-0">
          <AdminMessage tone="warning" density="compact" title="Network evidence requires attention" items={model.warnings} testId="session-network-warnings" />
        </div>
      {/if}
    </section>
  {/if}
</div>
