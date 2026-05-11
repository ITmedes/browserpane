<script lang="ts">
  import { Mic, Upload, Video } from 'lucide-svelte';
  import type { LiveSessionActionsViewModel } from './live-session-actions-view-model';

  type LiveSessionActionsPanelProps = {
    readonly viewModel: LiveSessionActionsViewModel;
    readonly onCameraToggle: () => void;
    readonly onMicrophoneToggle: () => void;
    readonly onUploadFiles: (files: FileList) => void;
  };

  let {
    viewModel,
    onCameraToggle,
    onMicrophoneToggle,
    onUploadFiles,
  }: LiveSessionActionsPanelProps = $props();
  let fileInput: HTMLInputElement | null = null;

  function chooseFiles(): void {
    fileInput?.click();
  }

  function handleUploadChange(event: Event): void {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files && input.files.length > 0) {
      onUploadFiles(input.files);
      input.value = '';
    }
  }
</script>

<section class="border-b border-[#90a6cc]/18 p-3" aria-label="Live session actions">
  <div class="grid min-w-0 grid-cols-3 gap-2 max-[520px]:grid-cols-1">
    <button
      class="inline-flex min-h-11 min-w-0 items-center justify-center gap-2 rounded-xl border border-[#90a6cc]/18 bg-admin-field px-2 text-xs font-bold text-admin-ink disabled:cursor-not-allowed disabled:opacity-45"
      type="button"
      data-testid="display-camera-toggle"
      disabled={viewModel.cameraDisabled}
      onclick={onCameraToggle}
    >
      <Video size={14} aria-hidden="true" />
      <span class="truncate">{viewModel.cameraLabel}</span>
    </button>
    <button
      class="inline-flex min-h-11 min-w-0 items-center justify-center gap-2 rounded-xl border border-[#90a6cc]/18 bg-admin-field px-2 text-xs font-bold text-admin-ink disabled:cursor-not-allowed disabled:opacity-45"
      type="button"
      data-testid="display-mic-toggle"
      disabled={viewModel.microphoneDisabled}
      onclick={onMicrophoneToggle}
    >
      <Mic size={14} aria-hidden="true" />
      <span class="truncate">{viewModel.microphoneLabel}</span>
    </button>
    <button
      class="inline-flex min-h-11 min-w-0 items-center justify-center gap-2 rounded-xl border border-[#90a6cc]/18 bg-admin-field px-2 text-xs font-bold text-admin-ink disabled:cursor-not-allowed disabled:opacity-45"
      type="button"
      data-testid="display-upload"
      disabled={viewModel.uploadDisabled}
      onclick={chooseFiles}
    >
      <Upload size={14} aria-hidden="true" />
      <span class="truncate">Upload files</span>
    </button>
  </div>

  <input class="sr-only" type="file" multiple data-testid="display-upload-input" bind:this={fileInput} onchange={handleUploadChange} />

  {#if viewModel.busy}
    <p class="admin-empty mt-3" data-testid="display-busy">Applying live action...</p>
  {/if}
  {#if viewModel.error}
    <p class="admin-error mt-3" data-testid="display-error">{viewModel.error}</p>
  {/if}
</section>
