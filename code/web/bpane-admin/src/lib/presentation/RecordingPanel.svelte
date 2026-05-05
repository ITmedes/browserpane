<script lang="ts">
  import type { RecordingViewModel } from './recording-view-model';

  type RecordingPanelProps = {
    readonly viewModel: RecordingViewModel;
    readonly autoDownload: boolean;
    readonly onAutoDownloadChange: (enabled: boolean) => void;
    readonly onStart: () => void;
    readonly onStop: () => void;
    readonly onDownload: () => void;
  };

  let {
    viewModel,
    autoDownload,
    onAutoDownloadChange,
    onStart,
    onStop,
    onDownload,
  }: RecordingPanelProps = $props();
</script>

<section class="grid gap-4" aria-label="Recording controls">
  <p class="m-0 text-sm leading-normal text-admin-ink/68">{viewModel.note}</p>

  <div class="grid grid-cols-3 gap-2 max-[760px]:grid-cols-1">
    <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-xs font-bold text-admin-ink/68 uppercase">
      Recording
      <strong class="mt-1 block text-admin-ink normal-case" data-testid="recording-status">{viewModel.status}</strong>
    </span>
    <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-xs font-bold text-admin-ink/68 uppercase">
      Session
      <strong class="mt-1 block font-mono text-admin-ink normal-case">{viewModel.sessionLabel}</strong>
    </span>
    <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-xs font-bold text-admin-ink/68 uppercase">
      Artifact
      <strong class="mt-1 block text-admin-ink normal-case">{viewModel.artifactLabel}</strong>
    </span>
  </div>

  <label class="flex items-start gap-3 rounded-[16px] bg-admin-leaf/10 p-3 text-sm font-bold text-admin-ink">
    <input
      class="mt-1"
      type="checkbox"
      checked={autoDownload}
      onchange={(event) => onAutoDownloadChange((event.currentTarget as HTMLInputElement).checked)}
    />
    <span>
      Auto download
      <small class="block font-normal text-admin-ink/62">Download the local WebM when recording stops.</small>
    </span>
  </label>

  <div class="flex flex-wrap gap-2">
    <button class="admin-button-primary" type="button" data-testid="recording-start" disabled={!viewModel.canStart} onclick={onStart}>
      Start Recording
    </button>
    <button class="admin-button-primary" type="button" data-testid="recording-stop" disabled={!viewModel.canStop} onclick={onStop}>
      Stop & Save WebM
    </button>
    <button class="admin-button-primary" type="button" data-testid="recording-download" disabled={!viewModel.canDownload} onclick={onDownload}>
      Download Last WebM
    </button>
  </div>

  {#if viewModel.busy}
    <p class="admin-empty mt-0">Recording operation in progress...</p>
  {/if}
  {#if viewModel.error}
    <p class="admin-error mt-0">{viewModel.error}</p>
  {/if}
</section>
