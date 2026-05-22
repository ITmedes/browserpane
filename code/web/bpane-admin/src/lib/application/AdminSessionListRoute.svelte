<script lang="ts">
  import { base } from '$app/paths';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import type { ControlClient } from '../api/control-client';
  import type {
    BrowserContextResource,
    CreateBrowserContextCommand,
    CreateSessionCommand,
    EgressProfileResource,
    SessionResource,
    SessionTemplateResource,
  } from '../api/control-types';
  import AdminMessage from '../presentation/AdminMessage.svelte';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import SessionCreateConfigurator from '../presentation/SessionCreateConfigurator.svelte';
  import { SessionViewModelBuilder, type SessionListItemViewModel } from '../presentation/session-view-model';
  import { ensureLocalEgressPresets } from './local-egress-presets';

  type AdminSessionListRouteProps = {
    readonly controlClient: ControlClient;
  };

  let { controlClient }: AdminSessionListRouteProps = $props();
  let sessions = $state<readonly SessionResource[]>([]);
  let sessionTemplates = $state<readonly SessionTemplateResource[]>([]);
  let browserContexts = $state<readonly BrowserContextResource[]>([]);
  let egressProfiles = $state<readonly EgressProfileResource[]>([]);
  let loading = $state(false);
  let templatesLoading = $state(false);
  let browserContextsLoading = $state(false);
  let egressProfilesLoading = $state(false);
  let error = $state<string | null>(null);
  let templateError = $state<string | null>(null);
  let browserContextError = $state<string | null>(null);
  let egressProfileError = $state<string | null>(null);
  let actionFeedback = $state<AdminMessageFeedback | null>(null);
  let search = $state('');
  let templateFilter = $state('');
  let stateFilter = $state('');
  let runtimeFilter = $state('');

  const viewModel = $derived(SessionViewModelBuilder.list({
    sessions,
    sessionTemplates,
    browserContexts,
    selectedSessionId: null,
    authenticated: true,
    loading,
    error,
  }));
  const filteredSessions = $derived(filterSessions(viewModel.sessions, search));

  onMount(() => {
    void loadSessionTemplates();
    void loadBrowserContexts();
    void loadEgressProfiles();
    void loadSessions(false);
  });

  async function loadSessionTemplates(): Promise<void> {
    templatesLoading = true;
    templateError = null;
    try {
      sessionTemplates = (await controlClient.listSessionTemplates()).templates;
    } catch (loadError) {
      templateError = errorMessage(loadError);
    } finally {
      templatesLoading = false;
    }
  }

  async function loadBrowserContexts(): Promise<void> {
    browserContextsLoading = true;
    browserContextError = null;
    try {
      browserContexts = (await controlClient.listBrowserContexts()).contexts;
    } catch (loadError) {
      browserContextError = errorMessage(loadError);
    } finally {
      browserContextsLoading = false;
    }
  }

  async function loadEgressProfiles(): Promise<void> {
    egressProfilesLoading = true;
    egressProfileError = null;
    try {
      const listed = (await controlClient.listEgressProfiles()).profiles;
      const localPresets = await ensureLocalEgressPresets(controlClient, listed);
      egressProfiles = localPresets.profiles;
      if (localPresets.error) {
        egressProfileError = localPresets.error;
      }
    } catch (loadError) {
      egressProfileError = errorMessage(loadError);
    } finally {
      egressProfilesLoading = false;
    }
  }

  async function loadSessions(showFeedback = true): Promise<void> {
    loading = true;
    error = null;
    actionFeedback = null;
    try {
      sessions = (await controlClient.listSessions({
        templateId: templateFilter || null,
        states: stateFilter ? [stateFilter] : [],
        runtimeStates: runtimeFilter ? [runtimeFilter] : [],
      })).sessions;
      if (showFeedback) {
        actionFeedback = successFeedback(`${sessions.length} session${sessions.length === 1 ? '' : 's'} refreshed.`);
      }
    } catch (loadError) {
      error = errorMessage(loadError);
      actionFeedback = null;
    } finally {
      loading = false;
    }
  }

  async function createSession(command: CreateSessionCommand): Promise<void> {
    loading = true;
    error = null;
    actionFeedback = null;
    try {
      const created = await controlClient.createSession(command);
      sessions = [created, ...sessions.filter((session) => session.id !== created.id)];
      void loadBrowserContexts();
      await goto(detailHref(created.id));
    } catch (createError) {
      error = errorMessage(createError);
    } finally {
      loading = false;
    }
  }

  async function createBrowserContext(command: CreateBrowserContextCommand): Promise<BrowserContextResource> {
    browserContextsLoading = true;
    browserContextError = null;
    actionFeedback = null;
    try {
      const created = await controlClient.createBrowserContext(command);
      browserContexts = [created, ...browserContexts.filter((context) => context.id !== created.id)];
      actionFeedback = {
        variant: 'success',
        title: 'Browser context saved',
        message: `Saved reusable browser context ${created.name}.`,
        testId: 'session-inspector-context-message',
      };
      return created;
    } catch (createError) {
      browserContextError = errorMessage(createError);
      actionFeedback = {
        variant: 'error',
        title: 'Browser context save failed',
        message: browserContextError,
        testId: 'session-inspector-context-message',
      };
      throw createError;
    } finally {
      browserContextsLoading = false;
    }
  }

  function detailHref(sessionId: string): string {
    return `${base}/sessions/${encodeURIComponent(sessionId)}`;
  }

  function filterSessions(
    items: readonly SessionListItemViewModel[],
    query: string,
  ): readonly SessionListItemViewModel[] {
    const normalized = query.trim().toLowerCase();
    if (!normalized) {
      return items;
    }
    return items.filter((session) => [
      session.id,
      session.shortId,
      session.lifecycle,
      session.runtime,
      session.presence,
      session.template,
      session.browserContext,
      session.networkIdentity,
      session.egress,
      session.mcpDelegation,
      session.labels,
    ].some((value) => value.toLowerCase().includes(normalized)));
  }

  function clientLabel(count: number): string {
    return count === 1 ? '1 client' : `${count} clients`;
  }

  function errorMessage(value: unknown): string {
    return value instanceof Error ? value.message : 'Unexpected session list error';
  }

  function successFeedback(message: string): AdminMessageFeedback {
    return { variant: 'success', title: 'Sessions refreshed', message, testId: 'session-inspector-list-message' };
  }
</script>

<section class="grid gap-5" data-testid="session-inspector-list">
  <div class="admin-panel mt-0">
    <div class="admin-header">
      <div>
        <p class="admin-eyebrow">Sessions</p>
        <h1 class="admin-section-title">Session inspector</h1>
      </div>
      <div class="admin-actions">
        <a class="admin-button-ghost" href={`${base}/`}>Live workspace</a>
        <a class="admin-button-ghost" href={`${base}/browser-contexts`}>Browser contexts</a>
        <a class="admin-button-ghost" href={`${base}/files/workspaces`}>File workspaces</a>
        <button
          class="admin-button-primary"
          type="button"
          data-testid="session-inspector-refresh"
          disabled={loading}
          onclick={() => void loadSessions()}
        >
          Refresh
        </button>
      </div>
    </div>
    <div class="mt-4 grid gap-3 md:grid-cols-[minmax(220px,360px)_1fr] md:items-end">
      <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
        Search
        <input
          class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="session-inspector-search"
          placeholder="Session id, state, runtime"
          bind:value={search}
        />
      </label>
      <div class="grid gap-3 sm:grid-cols-3">
        <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
          Template
          <select
            class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-inspector-template-filter"
            bind:value={templateFilter}
            disabled={loading || templatesLoading}
            onchange={() => void loadSessions(false)}
          >
            <option value="">All templates</option>
            {#each sessionTemplates as template}
              <option value={template.id}>{template.name}</option>
            {/each}
          </select>
        </label>
        <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
          State
          <select
            class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-inspector-state-filter"
            bind:value={stateFilter}
            disabled={loading}
            onchange={() => void loadSessions(false)}
          >
            <option value="">Any state</option>
            {#each ['pending', 'starting', 'ready', 'active', 'idle', 'released', 'stopping', 'stopped', 'failed', 'expired'] as state}
              <option value={state}>{state}</option>
            {/each}
          </select>
        </label>
        <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
          Runtime
          <select
            class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-inspector-runtime-filter"
            bind:value={runtimeFilter}
            disabled={loading}
            onchange={() => void loadSessions(false)}
          >
            <option value="">Any runtime</option>
            {#each ['not_started', 'starting', 'running', 'released', 'stopping', 'stopped'] as runtime}
              <option value={runtime}>{runtime}</option>
            {/each}
          </select>
        </label>
      </div>
      <p class="m-0 text-sm text-admin-ink/62 md:col-span-2" data-testid="session-inspector-count">
        {filteredSessions.length} of {viewModel.sessions.length} visible sessions
      </p>
    </div>
    {#if templateError}
      <div class="mt-3">
        <AdminMessage
          variant="warning"
          title="Template catalog unavailable"
          message={templateError}
          testId="session-inspector-template-error"
          compact={true}
        />
      </div>
    {/if}
  </div>

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
    loading={loading}
    submitTestId="session-inspector-new"
    submitLabel="Create and inspect"
    variant="panel"
    payloadInitiallyOpen={true}
    onCreateSession={(command) => void createSession(command)}
    onCreateBrowserContext={createBrowserContext}
  />

  {#if actionFeedback}
    <AdminMessage
      variant={actionFeedback.variant}
      title={actionFeedback.title}
      message={actionFeedback.message}
      testId={actionFeedback.testId}
      compact={true}
    />
  {/if}

  {#if loading && sessions.length === 0}
    <section class="admin-panel mt-0">
      <AdminMessage variant="loading" message="Loading sessions..." compact={true} />
    </section>
  {:else if error}
    <section class="admin-panel mt-0">
      <AdminMessage variant="error" message={error} testId="session-inspector-error" compact={true} />
    </section>
  {:else if filteredSessions.length === 0}
    <section class="admin-panel mt-0">
      <AdminMessage
        variant="empty"
        message="No sessions match the current filter."
        testId="session-inspector-empty"
        compact={true}
      />
    </section>
  {:else}
    <section class="grid gap-2" aria-label="Session table">
      {#each filteredSessions as session}
        <a
          class="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-4 rounded-xl border border-admin-ink/10 bg-admin-panel/82 p-4 text-admin-ink no-underline transition hover:border-admin-leaf/42 hover:bg-admin-field/78"
          href={detailHref(session.id)}
          data-testid="session-inspector-row"
          data-session-id={session.id}
        >
          <span class="grid min-w-0 gap-1">
            <strong class="truncate font-mono text-sm" title={session.id}>{session.id}</strong>
            <span class="truncate text-xs text-admin-ink/58">
              <span data-testid="session-inspector-row-template">{session.template}</span> | <span data-testid="session-inspector-row-browser-context">{session.browserContext}</span> | <span data-testid="session-inspector-row-network-identity">{session.networkIdentity}</span> | <span data-testid="session-inspector-row-egress">{session.egress}</span> | {session.mcpDelegation} | {session.labels} | updated {session.updatedAt}
            </span>
          </span>
          <span class="grid justify-items-end gap-1 text-xs text-[#c1d0e8]">
            <span class="rounded-lg bg-admin-field/72 px-2 py-1" data-testid="session-inspector-row-state">
              {session.lifecycle}
            </span>
            <span class="rounded-lg bg-admin-field/72 px-2 py-1">{clientLabel(session.clients)}</span>
          </span>
        </a>
      {/each}
    </section>
  {/if}
</section>
