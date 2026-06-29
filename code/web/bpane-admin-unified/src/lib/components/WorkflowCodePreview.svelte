<script lang="ts">
  import hljs from 'highlight.js/lib/core';
  import typescript from 'highlight.js/lib/languages/typescript';
  import 'highlight.js/styles/github-dark.css';
  import type { WorkflowSourcePreviewState } from '$lib/workflows/workflow-detail-state';
  import AdminMessage from './AdminMessage.svelte';

  if (!hljs.getLanguage('typescript')) {
    hljs.registerLanguage('typescript', typescript);
  }

  type WorkflowCodePreviewProps = {
    readonly state: WorkflowSourcePreviewState;
  };

  let { state }: WorkflowCodePreviewProps = $props();

  const highlightedCode = $derived(state.status === 'ready' ? highlightTypeScript(state.preview.content) : '');
  const languageLabel = $derived(state.status === 'ready'
    ? languageDisplayName(state.preview.language)
    : 'TypeScript');
  const byteSummary = $derived(state.status === 'ready'
    ? `${formatByteCount(state.preview.byte_count)}${state.preview.truncated ? ` of ${formatByteCount(state.preview.max_bytes)} previewed` : ''}`
    : '');

  function highlightTypeScript(source: string): string {
    return hljs.highlight(source, {
      language: 'typescript',
      ignoreIllegals: true,
    }).value;
  }

  function languageDisplayName(value: string): string {
    if (value === 'typescript') {
      return 'TypeScript';
    }
    if (value === 'json') {
      return 'JSON';
    }
    return 'Plain text';
  }

  function formatByteCount(value: number): string {
    if (value < 1024) {
      return `${value} B`;
    }
    return `${(value / 1024).toFixed(1)} KB`;
  }
</script>

<section class="min-w-0 rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="workflow-code-preview">
  <div class="flex flex-col gap-3 border-b border-admin-border pb-3 md:flex-row md:items-start md:justify-between">
    <div class="min-w-0">
      <h3 class="m-0 text-sm font-semibold text-admin-ink">Code preview</h3>
      <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
        Entrypoint source for the selected workflow version.
      </p>
    </div>
    <span
      class="inline-flex w-fit rounded-full bg-admin-panel px-2 py-0.5 text-xs font-semibold text-admin-ink ring-1 ring-admin-border"
      data-testid="workflow-code-preview-language"
    >
      {languageLabel}
    </span>
  </div>

  {#if state.status === 'idle'}
    <div class="mt-4">
      <AdminMessage
        tone="info"
        density="compact"
        title="No version selected"
        message="Select a workflow version to preview its source entrypoint."
        testId="workflow-code-preview-idle"
      />
    </div>
  {:else if state.status === 'loading'}
    <div class="mt-4">
      <AdminMessage
        tone="loading"
        density="compact"
        title="Loading source preview..."
        message={state.version}
        testId="workflow-code-preview-loading"
      />
    </div>
  {:else if state.status === 'unavailable'}
    <div class="mt-4">
      <AdminMessage
        tone="info"
        density="compact"
        title="Source preview unavailable"
        message={state.message}
        testId="workflow-code-preview-unavailable"
      />
    </div>
  {:else if state.status === 'error'}
    <div class="mt-4">
      <AdminMessage
        tone="error"
        density="compact"
        title="Source preview failed"
        message={state.message}
        testId="workflow-code-preview-error"
      />
    </div>
  {:else}
    <div class="mt-4 grid min-w-0 gap-3">
      <div class="flex min-w-0 flex-col gap-1 rounded-md border border-admin-border bg-admin-panel p-3 text-xs text-admin-muted md:flex-row md:items-center md:justify-between">
        <span class="min-w-0 break-words font-mono" data-testid="workflow-code-preview-entrypoint">
          {state.preview.entrypoint}
        </span>
        <span class="shrink-0" data-testid="workflow-code-preview-byte-count">{byteSummary}</span>
      </div>

      {#if state.preview.truncated}
        <AdminMessage
          tone="warning"
          density="compact"
          title="Preview truncated"
          message={`Only the first ${formatByteCount(state.preview.max_bytes)} are shown.`}
          testId="workflow-code-preview-truncated"
        />
      {/if}

      <div class="min-w-0 overflow-hidden rounded-md border border-slate-800 bg-slate-950 shadow-inner">
        <pre
          class="m-0 max-h-[520px] min-w-0 overflow-auto p-4 text-xs leading-5 text-slate-100"
          data-testid="workflow-code-preview-code"
        ><code class="hljs language-typescript" data-testid="workflow-code-preview-code-language">{@html highlightedCode}</code></pre>
      </div>
    </div>
  {/if}
</section>
