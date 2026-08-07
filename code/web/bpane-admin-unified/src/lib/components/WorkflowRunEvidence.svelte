<script lang="ts">
  import { Download } from '@lucide/svelte';
  import type { AdminActionState } from '$lib/application/admin-async-state';
  import { formatDateTime } from '$lib/projects/project-formatters';
  import {
    formatWorkflowRunBytes,
    formatWorkflowRunJson,
    type WorkflowRunDetailEvidenceState,
  } from '$lib/workflow-runs/workflow-run-detail-view-model';
  import type { WorkflowRunProducedFileResource } from '$lib/workflow-runs/workflow-run-types';
  import AdminMessage from './AdminMessage.svelte';
  import ActionFeedback from './ActionFeedback.svelte';

  type WorkflowRunEvidenceProps = {
    readonly evidence: WorkflowRunDetailEvidenceState;
    readonly downloadState?: AdminActionState;
    readonly onDownloadProducedFile?: (
      file: WorkflowRunProducedFileResource,
    ) => void | Promise<void>;
  };

  let {
    evidence,
    downloadState = { status: 'idle' },
    onDownloadProducedFile,
  }: WorkflowRunEvidenceProps = $props();
</script>

<section class="grid gap-5" data-testid="workflow-run-detail-evidence">
  <div class="grid gap-5 xl:grid-cols-2">
    <section class="min-w-0 border-t border-admin-border pt-4" data-testid="workflow-run-detail-events">
      <div class="flex items-end justify-between gap-3">
        <div>
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Timeline</p>
          <h2 class="m-0 mt-1 text-base font-semibold text-admin-ink">Events</h2>
        </div>
        {#if evidence.events.status === 'ready'}
          <span class="text-xs text-admin-muted" data-testid="workflow-run-detail-event-count">
            {evidence.events.items.length} events
          </span>
        {/if}
      </div>

      <div class="mt-3 max-h-96 min-h-28 overflow-auto rounded-md border border-admin-border bg-admin-panel">
        {#if evidence.events.status === 'loading' || evidence.events.status === 'idle'}
          <div class="p-3"><AdminMessage tone="loading" density="compact" title="Loading events" /></div>
        {:else if evidence.events.status === 'error'}
          <div class="p-3"><AdminMessage tone="error" density="compact" title="Events unavailable" message={evidence.events.message} testId="workflow-run-detail-events-error" /></div>
        {:else if evidence.events.items.length === 0}
          <p class="m-0 p-4 text-sm text-admin-muted">No workflow events are retained.</p>
        {:else}
          <ol class="m-0 list-none p-0">
            {#each [...evidence.events.items].reverse() as event}
              <li class="border-b border-admin-border p-3 last:border-b-0" data-testid="workflow-run-detail-event">
                <div class="flex min-w-0 flex-wrap items-baseline justify-between gap-2">
                  <strong class="min-w-0 break-words text-sm text-admin-ink">{event.event_type}</strong>
                  <span class="shrink-0 text-xs text-admin-muted">{formatDateTime(event.created_at)}</span>
                </div>
                <p class="m-0 mt-1 break-words text-sm leading-5 text-admin-muted">{event.message || 'No message'}</p>
                {#if event.data !== undefined && event.data !== null}
                  <pre class="m-0 mt-2 max-h-40 overflow-auto rounded-md bg-admin-soft p-2 text-xs leading-5 text-admin-ink">{formatWorkflowRunJson(event.data)}</pre>
                {/if}
              </li>
            {/each}
          </ol>
        {/if}
      </div>
    </section>

    <section class="min-w-0 border-t border-admin-border pt-4" data-testid="workflow-run-detail-logs">
      <div class="flex items-end justify-between gap-3">
        <div>
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Executor</p>
          <h2 class="m-0 mt-1 text-base font-semibold text-admin-ink">Logs</h2>
        </div>
        {#if evidence.logs.status === 'ready'}
          <span class="text-xs text-admin-muted" data-testid="workflow-run-detail-log-count">
            {evidence.logs.items.length} entries
          </span>
        {/if}
      </div>

      <div class="mt-3 max-h-96 min-h-28 overflow-auto rounded-md border border-admin-border bg-admin-night p-3 text-slate-100">
        {#if evidence.logs.status === 'loading' || evidence.logs.status === 'idle'}
          <p class="m-0 text-xs text-slate-300">Loading logs...</p>
        {:else if evidence.logs.status === 'error'}
          <AdminMessage tone="error" density="compact" title="Logs unavailable" message={evidence.logs.message} testId="workflow-run-detail-logs-error" />
        {:else if evidence.logs.items.length === 0}
          <p class="m-0 text-xs text-slate-300">No workflow logs are retained.</p>
        {:else}
          <ol class="m-0 grid list-none gap-2 p-0">
            {#each evidence.logs.items as log}
              <li class="min-w-0 font-mono text-xs leading-5" data-testid="workflow-run-detail-log">
                <span class="text-slate-400">{formatDateTime(log.created_at)}</span>
                <span class="ml-2 text-emerald-300">{log.source}/{log.stream}</span>
                <span class="ml-2 whitespace-pre-wrap break-words text-slate-100">{log.message}</span>
              </li>
            {/each}
          </ol>
        {/if}
      </div>
    </section>
  </div>

  <section class="min-w-0 border-t border-admin-border pt-4" data-testid="workflow-run-detail-produced-files">
    <div class="flex items-end justify-between gap-3">
      <div>
        <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Artifacts</p>
        <h2 class="m-0 mt-1 text-base font-semibold text-admin-ink">Produced files</h2>
      </div>
      {#if evidence.producedFiles.status === 'ready'}
        <span class="text-xs text-admin-muted" data-testid="workflow-run-detail-produced-file-count">
          {evidence.producedFiles.items.length} files
        </span>
      {/if}
    </div>

    <div class="mt-3 overflow-hidden rounded-md border border-admin-border bg-admin-panel">
      {#if evidence.producedFiles.status === 'loading' || evidence.producedFiles.status === 'idle'}
        <div class="p-3"><AdminMessage tone="loading" density="compact" title="Loading produced files" /></div>
      {:else if evidence.producedFiles.status === 'error'}
        <div class="p-3"><AdminMessage tone="error" density="compact" title="Produced files unavailable" message={evidence.producedFiles.message} testId="workflow-run-detail-produced-files-error" /></div>
      {:else if evidence.producedFiles.items.length === 0}
        <p class="m-0 p-4 text-sm text-admin-muted">This run has no produced files.</p>
      {:else}
        <ul class="m-0 list-none divide-y divide-admin-border p-0">
          {#each evidence.producedFiles.items as file}
            <li class="flex min-w-0 flex-col gap-3 p-3 sm:flex-row sm:items-center sm:justify-between" data-testid="workflow-run-detail-produced-file">
              <div class="min-w-0">
                <p class="m-0 truncate text-sm font-semibold text-admin-ink" title={file.file_name}>{file.file_name}</p>
                <p class="m-0 mt-1 break-words font-mono text-xs text-admin-muted">
                  {formatWorkflowRunBytes(file.byte_count)} / {file.media_type ?? 'application/octet-stream'} / {file.sha256_hex}
                </p>
              </div>
              <button
                class="inline-flex h-9 shrink-0 items-center justify-center gap-2 rounded-md border border-admin-border bg-white px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
                type="button"
                onclick={() => void onDownloadProducedFile?.(file)}
                data-testid="workflow-run-detail-download-produced-file"
              >
                <Download size={15} strokeWidth={1.9} aria-hidden="true" />
                <span>Download</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
    <div class="mt-3">
      <ActionFeedback
        state={downloadState}
        successTitle="Download ready"
        errorTitle="Produced file download failed"
        successTestId="workflow-run-detail-download-success"
        errorTestId="workflow-run-detail-download-error"
        runningTestId="workflow-run-detail-download-running"
      />
    </div>
  </section>
</section>
