<script lang="ts">
  import { Activity, Bot, Files, Gauge, Network, ShieldCheck, Video } from '@lucide/svelte';
  import {
    sessionSubareaHref,
    sessionSubareas,
    type SessionSubareaId,
  } from '$lib/sessions/session-subarea';

  type SessionSubareaNavigationProps = {
    readonly sessionId: string;
    readonly activeId: SessionSubareaId;
    readonly availableIds?: readonly SessionSubareaId[];
  };

  let {
    sessionId,
    activeId,
    availableIds = ['overview', 'live'],
  }: SessionSubareaNavigationProps = $props();

  const visibleSubareas = $derived(sessionSubareas.filter((subarea) => availableIds.includes(subarea.id)));
</script>

<nav class="min-w-0 border-b border-admin-border" aria-label="Session areas" data-testid="session-subarea-navigation">
  <div class="flex min-w-0 gap-1 overflow-x-auto" role="list">
    {#each visibleSubareas as subarea}
      <a
        class={`inline-flex h-10 shrink-0 items-center gap-2 border-b-2 px-3 text-sm font-medium transition-colors ${activeId === subarea.id
          ? 'border-admin-accent text-admin-ink'
          : 'border-transparent text-admin-muted hover:border-admin-border hover:text-admin-ink'}`}
        href={sessionSubareaHref(sessionId, subarea.id)}
        aria-current={activeId === subarea.id ? 'page' : undefined}
        data-testid={`session-subarea-${subarea.id}`}
      >
        {#if subarea.id === 'overview'}
          <Gauge size={15} strokeWidth={1.8} />
        {:else if subarea.id === 'live'}
          <Activity size={15} strokeWidth={1.8} />
        {:else if subarea.id === 'automation'}
          <Bot size={15} strokeWidth={1.8} />
        {:else if subarea.id === 'policy'}
          <ShieldCheck size={15} strokeWidth={1.8} />
        {:else if subarea.id === 'files'}
          <Files size={15} strokeWidth={1.8} />
        {:else if subarea.id === 'recordings'}
          <Video size={15} strokeWidth={1.8} />
        {:else}
          <Network size={15} strokeWidth={1.8} />
        {/if}
        <span>{subarea.label}</span>
      </a>
    {/each}
  </div>
</nav>
