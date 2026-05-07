<script lang="ts">
  import type { RecordingSegmentCardViewModel } from './recording-view-model';

  type RecordingSegmentCardProps = {
    readonly viewModel: RecordingSegmentCardViewModel;
    readonly onDownload: (recordingId: string) => void;
  };

  let { viewModel, onDownload }: RecordingSegmentCardProps = $props();
</script>

<article
  class="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-x-4 gap-y-3 rounded-[16px] border border-admin-ink/10 bg-admin-cream/72 p-4 max-[760px]:grid-cols-1"
  data-testid="recording-library-row"
  data-recording-id={viewModel.id}
>
  <div class="flex min-w-0 flex-wrap items-center gap-3">
    <strong class="[overflow-wrap:anywhere]">{viewModel.title}</strong>
    <span class="rounded-full bg-admin-leaf/12 px-2.5 py-1 text-xs font-bold text-admin-leaf">
      {viewModel.state}
    </span>
  </div>
  <button
    class="admin-button-primary"
    type="button"
    data-testid="recording-segment-download"
    data-action="download-recording"
    data-recording-id={viewModel.id}
    disabled={!viewModel.canDownload}
    onclick={() => onDownload(viewModel.id)}
  >
    {viewModel.downloadLabel}
  </button>
  <div class="flex min-w-0 flex-wrap gap-2 text-sm text-admin-ink/68">
    {#each viewModel.metadata as item}
      <span>{item}</span>
    {/each}
  </div>
  {#if viewModel.error}
    <p class="admin-error col-span-full m-0">{viewModel.error}</p>
  {/if}
</article>
