<script lang="ts">
  import { AlertCircle, AlertTriangle, CheckCircle2, Info, LoaderCircle } from '@lucide/svelte';

  export type AdminMessageTone = 'info' | 'success' | 'warning' | 'error' | 'loading';
  export type AdminMessageDensity = 'regular' | 'compact';

  type AdminMessageProps = {
    readonly tone?: AdminMessageTone;
    readonly density?: AdminMessageDensity;
    readonly title?: string;
    readonly message?: string;
    readonly items?: readonly string[];
    readonly testId?: string;
  };

  let {
    tone = 'info',
    density = 'regular',
    title,
    message,
    items = [],
    testId,
  }: AdminMessageProps = $props();

  const shellClass = $derived(toneShellClass(tone));
  const role = $derived(tone === 'error' ? 'alert' : 'status');
  const ariaLive = $derived(tone === 'error' || tone === 'warning' ? 'assertive' : 'polite');
  const spacingClass = $derived(density === 'compact' ? 'gap-2 px-3 py-2 text-xs' : 'gap-3 p-3 text-sm');
  const iconSize = $derived(density === 'compact' ? 15 : 17);

  function toneShellClass(value: AdminMessageTone): string {
    if (value === 'success') {
      return 'border-emerald-200 bg-emerald-50 text-emerald-900';
    }
    if (value === 'warning') {
      return 'border-amber-200 bg-amber-50 text-amber-900';
    }
    if (value === 'error') {
      return 'border-red-200 bg-red-50 text-red-900';
    }
    if (value === 'loading') {
      return 'border-admin-border bg-admin-soft text-admin-muted';
    }
    return 'border-blue-200 bg-blue-50 text-blue-900';
  }
</script>

<div
  class={`flex items-start rounded-md border ${spacingClass} ${shellClass}`}
  role={role}
  aria-live={ariaLive}
  data-testid={testId}
>
  <span class="mt-0.5 shrink-0" aria-hidden="true">
    {#if tone === 'success'}
      <CheckCircle2 size={iconSize} strokeWidth={1.9} />
    {:else if tone === 'warning'}
      <AlertTriangle size={iconSize} strokeWidth={1.9} />
    {:else if tone === 'error'}
      <AlertCircle size={iconSize} strokeWidth={1.9} />
    {:else if tone === 'loading'}
      <LoaderCircle class="animate-spin" size={iconSize} strokeWidth={1.9} />
    {:else}
      <Info size={iconSize} strokeWidth={1.9} />
    {/if}
  </span>

  <div class="min-w-0">
    {#if title}
      <p class="m-0 font-semibold">{title}</p>
    {/if}
    {#if message}
      <p class={`m-0 break-words ${title ? 'mt-1' : ''}`}>{message}</p>
    {/if}
    {#if items.length > 0}
      <ul class={`m-0 list-disc pl-4 ${title || message ? 'mt-1' : ''}`}>
        {#each items as item}
          <li class="break-words">{item}</li>
        {/each}
      </ul>
    {/if}
  </div>
</div>
