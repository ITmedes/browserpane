<script lang="ts">
  import { base } from '$app/paths';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import type { ControlClient } from '../api/control-client';
  import type { CreateSessionCommand, SessionResource } from '../api/control-types';
  import AdminMessage from '../presentation/AdminMessage.svelte';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import SessionCreateConfigurator from '../presentation/SessionCreateConfigurator.svelte';
  import { SessionViewModelBuilder, type SessionListItemViewModel } from '../presentation/session-view-model';

  type AdminSessionListRouteProps = {
    readonly controlClient: ControlClient;
  };

  let { controlClient }: AdminSessionListRouteProps = $props();
  let sessions = $state<readonly SessionResource[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let actionFeedback = $state<AdminMessageFeedback | null>(null);
  let search = $state('');

  const viewModel = $derived(SessionViewModelBuilder.list({
    sessions,
    selectedSessionId: null,
    authenticated: true,
    loading,
    error,
  }));
  const filteredSessions = $derived(filterSessions(viewModel.sessions, search));

  onMount(() => {
    void loadSessions(false);
  });

  async function loadSessions(showFeedback = true): Promise<void> {
    loading = true;
    error = null;
    actionFeedback = null;
    try {
      sessions = (await controlClient.listSessions()).sessions;
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
      await goto(detailHref(created.id));
    } catch (createError) {
      error = errorMessage(createError);
    } finally {
      loading = false;
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
      <p class="m-0 text-sm text-admin-ink/62" data-testid="session-inspector-count">
        {filteredSessions.length} of {viewModel.sessions.length} visible sessions
      </p>
    </div>
  </div>

  <SessionCreateConfigurator
    loading={loading}
    submitTestId="session-inspector-new"
    submitLabel="Create and inspect"
    variant="panel"
    payloadInitiallyOpen={true}
    onCreateSession={(command) => void createSession(command)}
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
              {session.mcpDelegation} | {session.labels} | updated {session.updatedAt}
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
