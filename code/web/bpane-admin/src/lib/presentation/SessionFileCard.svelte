<script lang="ts">
  import type { SessionFileCardViewModel } from './session-file-view-model';

  type SessionFileCardProps = {
    readonly viewModel: SessionFileCardViewModel;
    readonly downloadingFileId: string | null;
    readonly onDownload: (fileId: string) => void;
  };

  let { viewModel, downloadingFileId, onDownload }: SessionFileCardProps = $props();
</script>

<article
  class="grid grid-cols-[minmax(0,1fr)_auto] gap-x-4 gap-y-2.5 rounded-[18px] border border-admin-ink/10 bg-admin-cream/72 p-4 max-[760px]:grid-cols-1"
  data-testid="session-files-row"
>
  <div class="flex min-w-0 flex-wrap items-baseline gap-3">
    <strong class="[overflow-wrap:anywhere]">{viewModel.name}</strong>
    <span class="text-admin-ink/68">{viewModel.source}</span>
  </div>
  <div class="flex min-w-0 flex-wrap items-baseline gap-3 text-admin-ink/68">
    <span>{viewModel.size}</span>
    <span>{viewModel.mediaType}</span>
    <span>{viewModel.createdAt}</span>
  </div>
  <div class="font-mono text-[0.82rem] text-admin-ink/68">{viewModel.digest}</div>
  <button
    class="admin-button-primary"
    type="button"
    data-testid="session-file-download"
    disabled={Boolean(downloadingFileId)}
    onclick={() => onDownload(viewModel.id)}
  >
    {downloadingFileId === viewModel.id ? 'Downloading...' : 'Download'}
  </button>
</article>
