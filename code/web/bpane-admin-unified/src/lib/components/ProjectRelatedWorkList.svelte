<script lang="ts">
  import { ArrowUpRight } from '@lucide/svelte';
  import { formatDateTime } from '$lib/projects/project-formatters';
  import type { ProjectRelatedWorkItem } from '$lib/projects/project-governance-types';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';

  type ProjectRelatedWorkListProps = {
    readonly title: string;
    readonly emptyMessage: string;
    readonly items: readonly ProjectRelatedWorkItem[];
    readonly status: 'idle' | 'loading' | 'ready' | 'error';
    readonly errorMessage?: string | null;
    readonly testId: string;
  };

  let {
    title,
    emptyMessage,
    items,
    status,
    errorMessage = null,
    testId,
  }: ProjectRelatedWorkListProps = $props();
</script>

<section class="min-w-0" data-testid={testId}>
  <div class="flex items-center justify-between gap-3">
    <h5 class="m-0 text-sm font-semibold text-admin-ink">{title}</h5>
    {#if status === 'ready'}
      <span class="text-xs font-semibold text-admin-muted">{items.length}</span>
    {/if}
  </div>

  {#if status === 'error'}
    <div class="mt-3">
      <AdminMessage
        tone="warning"
        density="compact"
        title={`${title} unavailable`}
        message={errorMessage ?? 'Related work request failed.'}
      />
    </div>
  {:else if status !== 'ready'}
    <p class="m-0 mt-3 text-sm text-admin-muted">Loading {title.toLowerCase()}...</p>
  {:else if items.length === 0}
    <p class="m-0 mt-3 text-sm text-admin-muted">{emptyMessage}</p>
  {:else}
    <ul class="m-0 mt-3 grid list-none gap-2 p-0">
      {#each items as item}
        <li class="border-l-2 border-admin-border pl-3">
          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <a class="min-w-0 break-all text-sm font-semibold text-admin-accent hover:underline" href={item.href}>
              {item.id}
            </a>
            <ArrowUpRight class="shrink-0 text-admin-muted" size={14} strokeWidth={1.8} />
            <span class={`rounded-full px-2 py-0.5 text-[11px] font-semibold ring-1 ${projectToneClass(item.tone)}`}>
              {item.state}
            </span>
          </div>
          <div class="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-xs text-admin-muted">
            {#if item.admissionState}<span>Admission: {item.admissionState}</span>{/if}
            {#if item.reasonCode}<span>Reason: {item.reasonCode}</span>{/if}
            {#if item.queuePosition !== null}<span>Queue: #{item.queuePosition}</span>{/if}
            {#if item.queuedAt}<span>Queued: {formatDateTime(item.queuedAt)}</span>{/if}
            <span>Updated: {formatDateTime(item.updatedAt)}</span>
          </div>
          {#if item.message}
            <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">{item.message}</p>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>
