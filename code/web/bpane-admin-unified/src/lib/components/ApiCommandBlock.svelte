<script lang="ts">
  import { Check, Copy } from '@lucide/svelte';
  import hljs from 'highlight.js/lib/core';
  import bash from 'highlight.js/lib/languages/bash';
  import 'highlight.js/styles/github-dark.css';

  if (!hljs.getLanguage('bash')) {
    hljs.registerLanguage('bash', bash);
  }

  type CopyState = 'idle' | 'success' | 'error';

  type ApiCommandBlockProps = {
    readonly command: string;
    readonly label: string;
    readonly testId: string;
  };

  let { command, label, testId }: ApiCommandBlockProps = $props();
  let copyState = $state<CopyState>('idle');
  const highlighted = $derived(hljs.highlight(command, { language: 'bash', ignoreIllegals: true }).value);

  async function copyCommand(): Promise<void> {
    try {
      await navigator.clipboard.writeText(command);
      copyState = 'success';
    } catch {
      copyState = 'error';
    }
  }
</script>

<div class="min-w-0 overflow-hidden rounded-md border border-slate-800 bg-slate-950 shadow-inner" data-testid={testId}>
  <div class="flex min-h-10 items-center justify-between gap-3 border-b border-slate-800 px-3">
    <span class="min-w-0 truncate text-xs font-semibold text-slate-300">{label}</span>
    <button
      type="button"
      class="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-slate-300 hover:bg-slate-800 hover:text-white focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-admin-accent"
      aria-label={copyState === 'success' ? 'Command copied' : 'Copy command'}
      title={copyState === 'success' ? 'Copied' : 'Copy command'}
      data-testid={`${testId}-copy`}
      onclick={() => void copyCommand()}
    >
      {#if copyState === 'success'}
        <Check size={16} strokeWidth={2} />
      {:else}
        <Copy size={16} strokeWidth={2} />
      {/if}
    </button>
  </div>
  <pre class="m-0 max-h-[420px] min-w-0 overflow-auto p-4 text-xs leading-5 text-slate-100"><code class="hljs language-bash">{@html highlighted}</code></pre>
  <div class="min-h-7 px-3 pb-2 text-xs" aria-live="polite" data-testid={`${testId}-feedback`}>
    {#if copyState === 'success'}
      <span class="font-medium text-emerald-300">Command copied to clipboard.</span>
    {:else if copyState === 'error'}
      <span class="font-medium text-rose-300">Clipboard access failed. Select the command manually.</span>
    {/if}
  </div>
</div>
