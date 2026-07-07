<script lang="ts">
  import { Download, Search } from '@lucide/svelte';
  import {
    buildRecordingOverviewModel,
    isActiveRecording,
    recordingMatchesSearch,
    type RecordingOverviewRow,
  } from '$lib/recordings/recording-overview-view-model';
  import type { RecordingCatalogEntry } from '$lib/recordings/recording-types';
  import { projectToneClass } from '$lib/projects/project-ui';

  type RecordingLens = 'all' | 'downloadable' | 'active' | 'failed' | 'missing';

  type RecordingLensDefinition = {
    readonly id: RecordingLens;
    readonly label: string;
    readonly count: number;
    readonly showDot?: boolean;
  };

  type RecordingTableItem = {
    readonly entry: RecordingCatalogEntry;
    readonly row: RecordingOverviewRow;
  };

  type RecordingCatalogTableProps = {
    readonly entries: readonly RecordingCatalogEntry[];
    readonly busy?: boolean;
    readonly onDownloadRecording?: (entry: RecordingCatalogEntry) => void | Promise<void>;
  };

  let {
    entries,
    busy = false,
    onDownloadRecording,
  }: RecordingCatalogTableProps = $props();
  let recordingLens = $state<RecordingLens>('all');
  let searchQuery = $state('');

  const model = $derived(buildRecordingOverviewModel(entries));
  const items = $derived(entries.map((entry, index) => ({ entry, row: model.rows[index]! })));
  const lensDefinitions = $derived(buildLensDefinitions(items));
  const visibleItems = $derived(filterItems(items, recordingLens, searchQuery));

  function buildLensDefinitions(rows: readonly RecordingTableItem[]): readonly RecordingLensDefinition[] {
    return [
      { id: 'all', label: 'All', count: rows.length },
      {
        id: 'downloadable',
        label: 'Downloadable',
        count: rows.filter((item) => item.row.canDownload).length,
        showDot: true,
      },
      { id: 'active', label: 'Active', count: rows.filter((item) => isActiveRecording(item.entry.recording)).length },
      { id: 'failed', label: 'Failed', count: rows.filter((item) => item.entry.recording.state === 'failed').length },
      {
        id: 'missing',
        label: 'Missing artifact',
        count: rows.filter((item) => item.entry.recording.state === 'ready' && !item.entry.recording.artifact_available).length,
      },
    ];
  }

  function filterItems(
    rows: readonly RecordingTableItem[],
    lens: RecordingLens,
    query: string,
  ): readonly RecordingTableItem[] {
    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter((item) => matchesLens(item, lens) && recordingMatchesSearch(item.row, normalizedQuery));
  }

  function matchesLens(item: RecordingTableItem, lens: RecordingLens): boolean {
    if (lens === 'downloadable') {
      return item.row.canDownload;
    }
    if (lens === 'active') {
      return isActiveRecording(item.entry.recording);
    }
    if (lens === 'failed') {
      return item.entry.recording.state === 'failed';
    }
    if (lens === 'missing') {
      return item.entry.recording.state === 'ready' && !item.entry.recording.artifact_available;
    }
    return true;
  }

  function downloadRecording(entry: RecordingCatalogEntry): void {
    void onDownloadRecording?.(entry);
  }

  function sessionHref(sessionId: string): string {
    return `/admin-new/sessions/${encodeURIComponent(sessionId)}`;
  }
</script>

<section class="min-h-0 min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="recordings-list">
  <div class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
    <div class="min-w-0">
      <h2 class="m-0 text-sm font-semibold text-admin-ink">Recording catalog</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">Session-scoped recording segments, artifacts, playback exports, and failure state.</p>
    </div>
    <span class="text-xs text-admin-muted" data-testid="recordings-list-count">
      {visibleItems.length} of {model.rows.length}
    </span>
  </div>

  <div class="flex flex-col gap-0 border-b border-admin-border bg-admin-panel lg:flex-row lg:items-center lg:justify-between">
    <div class="flex min-w-0 flex-wrap items-center gap-0 px-4">
      {#each lensDefinitions as lens}
        <button
          class={`inline-flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium ${
            recordingLens === lens.id
              ? 'border-admin-accent text-admin-ink'
              : 'border-transparent text-admin-muted hover:text-admin-ink'
          }`}
          type="button"
          aria-pressed={recordingLens === lens.id}
          onclick={() => {
            recordingLens = lens.id;
          }}
          data-testid={`recordings-lens-${lens.id}`}
        >
          {#if lens.showDot}
            <span class="h-1.5 w-1.5 rounded-full bg-admin-success" aria-hidden="true"></span>
          {/if}
          <span>{lens.label}</span>
          <span class="rounded border border-admin-border bg-admin-soft px-1.5 py-0.5 text-[11px] font-semibold text-admin-muted">
            {lens.count}
          </span>
        </button>
      {/each}
    </div>

    <label class="mx-4 mb-3 flex h-9 min-w-0 items-center gap-2 rounded-md border border-admin-border px-3 text-sm text-admin-muted lg:mb-0 lg:w-[360px]">
      <Search size={15} strokeWidth={1.8} aria-hidden="true" />
      <span class="sr-only">Search recordings</span>
      <input
        class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none placeholder:text-admin-muted"
        type="search"
        placeholder="Recording, session, project, state..."
        bind:value={searchQuery}
        data-testid="recordings-search"
      />
    </label>
  </div>

  <div class="max-h-[calc(100vh-360px)] min-h-64 overflow-auto bg-admin-panel">
    <table class="w-full min-w-[1120px] border-collapse">
      <thead class="sticky top-0 z-10 bg-admin-soft">
        <tr class="border-b border-admin-border">
          <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Recording</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Session</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">State</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Artifact</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Media</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Duration</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Completed</th>
          <th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted" scope="col">Actions</th>
        </tr>
      </thead>
      <tbody>
        {#if visibleItems.length === 0}
          <tr>
            <td class="px-4 py-14 text-center text-sm text-admin-muted" colspan="8" data-testid="recordings-filter-empty">
              No recordings match the current filters.
            </td>
          </tr>
        {:else}
          {#each visibleItems as item}
            {@const row = item.row}
            <tr class="border-b border-admin-border last:border-b-0 hover:bg-admin-soft" data-testid="recordings-list-row">
              <td class="w-[260px] px-4 py-3 align-middle">
                <div class="grid min-w-0 text-left">
                  <span class="truncate font-mono text-sm font-semibold text-admin-ink" title={row.id}>{row.shortId}</span>
                  <span class="mt-1 truncate font-mono text-[11px] text-admin-muted">{row.id}</span>
                </div>
              </td>
              <td class="max-w-[230px] px-3 py-3 align-middle text-xs text-admin-muted">
                <a
                  class="block truncate text-sm font-semibold text-admin-ink hover:text-admin-accent"
                  href={sessionHref(row.sessionId)}
                  data-testid="recordings-session-link"
                >
                  {row.sessionLabel}
                </a>
                <span class="mt-1 block truncate font-mono">{row.shortSessionId}</span>
                <span class="mt-1 block truncate">{row.project}</span>
              </td>
              <td class="px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.stateTone)}`}>
                  {row.state}
                </span>
              </td>
              <td class="px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.artifactTone)}`}>
                  {row.artifactLabel}
                </span>
              </td>
              <td class="max-w-[190px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="block font-semibold text-admin-ink">{row.size}</span>
                <span class="block truncate">{row.format} · {row.mimeType}</span>
              </td>
              <td class="px-3 py-3 align-middle text-xs text-admin-muted">{row.duration}</td>
              <td class="max-w-[190px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="block">{row.completedAt}</span>
                <span class="mt-1 block truncate">{row.termination}</span>
              </td>
              <td class="px-4 py-3 align-middle">
                <div class="flex justify-end gap-2">
                  <button
                    class="inline-flex h-8 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
                    type="button"
                    onclick={() => downloadRecording(item.entry)}
                    disabled={busy || !row.canDownload}
                    title={row.downloadDescription}
                    data-testid="recordings-download"
                  >
                    <Download size={14} strokeWidth={1.8} />
                    <span>Download</span>
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>
</section>
