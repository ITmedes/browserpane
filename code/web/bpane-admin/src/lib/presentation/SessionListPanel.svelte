<script lang="ts">
  import { base } from '$app/paths';
  import type {
    BrowserContextResource,
    CreateBrowserContextCommand,
    CreateSessionCommand,
    EgressProfileResource,
    SessionTemplateResource,
  } from '../api/control-types';
  import AdminMessage from './AdminMessage.svelte';
  import SessionCreateConfigurator from './SessionCreateConfigurator.svelte';
  import SessionTable from './SessionTable.svelte';
  import type { SessionListPanelViewModel } from './session-view-model';

  type SessionListPanelProps = {
    readonly viewModel: SessionListPanelViewModel;
    readonly sessionTemplates?: readonly SessionTemplateResource[];
    readonly browserContexts?: readonly BrowserContextResource[];
    readonly egressProfiles?: readonly EgressProfileResource[];
    readonly templatesLoading?: boolean;
    readonly browserContextsLoading?: boolean;
    readonly egressProfilesLoading?: boolean;
    readonly templateError?: string | null;
    readonly browserContextError?: string | null;
    readonly egressProfileError?: string | null;
    readonly connected: boolean;
    readonly onRefresh: () => void;
    readonly onCreateSession: (command: CreateSessionCommand) => void;
    readonly onCreateBrowserContext?: (command: CreateBrowserContextCommand) => Promise<BrowserContextResource | void>;
    readonly onJoinSession: () => void;
    readonly onDisconnectSession: () => void;
    readonly onSelectSessionId: (sessionId: string) => void;
  };

  let {
    viewModel,
    sessionTemplates = [],
    browserContexts = [],
    egressProfiles = [],
    templatesLoading = false,
    browserContextsLoading = false,
    egressProfilesLoading = false,
    templateError = null,
    browserContextError = null,
    egressProfileError = null,
    connected,
    onRefresh,
    onCreateSession,
    onCreateBrowserContext,
    onJoinSession,
    onDisconnectSession,
    onSelectSessionId,
  }: SessionListPanelProps = $props();
  let createPayloadOpen = $state(false);

  function detailHref(sessionId: string): string {
    return `${base}/sessions/${encodeURIComponent(sessionId)}`;
  }
</script>

<div class="grid min-w-0 gap-3" aria-label="Owner-scoped sessions">
  <section class="grid gap-3 rounded-[16px] border border-admin-leaf/25 bg-admin-leaf/10 p-3" aria-label="Selected session">
    <div class="flex min-w-0 flex-wrap items-start justify-between gap-2">
      <div class="min-w-0">
        <p class="admin-eyebrow mb-1">Selected session</p>
        <h3 class="m-0 truncate font-mono text-sm font-extrabold text-admin-ink" title={viewModel.selectedSession?.id ?? ''}>
          {viewModel.selectedSession?.id ?? 'No session selected'}
        </h3>
      </div>
      <span class="rounded-full border border-admin-leaf/30 bg-admin-leaf/12 px-3 py-1 text-xs font-extrabold text-admin-leaf">
        {viewModel.selectedSession?.presence ?? 'select'}
      </span>
    </div>

    {#if viewModel.selectedSession}
      <div class="grid min-w-0 grid-cols-2 gap-2 text-xs text-admin-ink/70 sm:grid-cols-3">
        {@render Fact('State', viewModel.selectedSession.lifecycle, 'session-selected-state')}
        {@render Fact('Template', viewModel.selectedSession.template, 'session-selected-template')}
        {@render Fact('Context', viewModel.selectedSession.browserContext, 'session-selected-browser-context')}
        {@render Fact('Network', viewModel.selectedSession.networkIdentity, 'session-selected-network-identity')}
        {@render Fact('Egress', viewModel.selectedSession.egress, 'session-selected-egress')}
        {@render Fact('Runtime', viewModel.selectedSession.runtime, 'session-selected-runtime')}
        {@render Fact('Clients', String(viewModel.selectedSession.clients), 'session-selected-clients')}
        {@render Fact('MCP', viewModel.selectedSession.mcpDelegation, 'session-selected-mcp')}
      </div>
      <p class="m-0 truncate text-xs text-admin-ink/58">
        {viewModel.selectedSession.ownerMode} | {viewModel.selectedSession.runtimeBinding} | {viewModel.selectedSession.labels} | updated {viewModel.selectedSession.updatedAt}
      </p>
    {:else}
      <p class="m-0 text-sm leading-normal text-admin-ink/68">
        Select an existing session or create a new one before joining, delegating MCP, or inspecting runtime state.
      </p>
    {/if}

    <div class="flex flex-wrap gap-2">
      <button
        class="admin-button-primary"
        type="button"
        data-testid="session-join"
        disabled={!viewModel.authenticated || viewModel.loading || !viewModel.selectedSessionId || !viewModel.selectedSession?.canJoin}
        onclick={onJoinSession}
      >
        Start / reconnect
      </button>
      <button
        class="admin-button-ghost"
        type="button"
        data-testid="session-disconnect"
        disabled={!viewModel.authenticated || viewModel.loading || !viewModel.selectedSessionId || !connected}
        onclick={onDisconnectSession}
      >
        Disconnect
      </button>
      {#if viewModel.selectedSessionId}
        <a class="admin-button-ghost" data-testid="session-detail-link" href={detailHref(viewModel.selectedSessionId)}>
          Inspect details
        </a>
      {/if}
    </div>
  </section>

  <SessionCreateConfigurator
    {sessionTemplates}
    {browserContexts}
    {egressProfiles}
    {templatesLoading}
    {browserContextsLoading}
    {egressProfilesLoading}
    {templateError}
    {browserContextError}
    {egressProfileError}
    loading={viewModel.loading}
    disabled={!viewModel.authenticated}
    submitTestId="session-new"
    variant="inline"
    payloadOpen={createPayloadOpen}
    onPayloadOpenChange={(open) => { createPayloadOpen = open; }}
    {onCreateSession}
    {onCreateBrowserContext}
  />

  <section class="grid gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Session switcher">
    <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
      <div>
        <p class="admin-eyebrow mb-1">Session switcher</p>
        <p class="m-0 text-sm font-bold text-admin-ink/72">{viewModel.sessions.length} visible sessions</p>
      </div>
      <div class="flex flex-wrap gap-2">
        <button
          class="admin-button-primary"
          type="button"
          data-testid="session-refresh"
          disabled={!viewModel.authenticated || viewModel.loading}
          onclick={onRefresh}
        >
          Refresh
        </button>
      </div>
    </div>

    {#if !viewModel.authenticated}
      <AdminMessage
        variant="warning"
        title="Sign in required"
        message="Sign in to inspect sessions from /api/v1/sessions."
        compact={true}
      />
    {:else if viewModel.loading}
      <AdminMessage variant="loading" message="Loading sessions..." compact={true} />
    {:else if viewModel.error}
      <AdminMessage variant="error" message={viewModel.error} compact={true} />
    {:else if viewModel.sessions.length === 0}
      <AdminMessage
        variant="empty"
        message="No owner-scoped sessions are visible for this operator."
        compact={true}
      />
    {:else}
      <SessionTable
        sessions={viewModel.sessions}
        selectedSessionId={viewModel.selectedSessionId}
        {onSelectSessionId}
      />
    {/if}
  </section>
</div>

{#snippet Fact(label: string, value: string, testId: string)}
  <span class="min-w-0 rounded-xl bg-admin-field/72 p-2 font-bold uppercase">
    {label}
    <strong class="mt-1 block truncate font-mono text-admin-ink normal-case" data-testid={testId}>{value}</strong>
  </span>
{/snippet}
