<script lang="ts">
  import type { BrowserSessionRenderBackend } from '../session/browser-session-types';
  import {
    DISPLAY_BACKEND_OPTIONS,
    type DisplaySettingsViewModel,
  } from './display-settings-view-model';

  type DisplayControlsPanelProps = {
    readonly viewModel: DisplaySettingsViewModel;
    readonly onRenderBackendChange: (value: BrowserSessionRenderBackend) => void;
    readonly onHiDpiChange: (value: boolean) => void;
    readonly onScrollCopyChange: (value: boolean) => void;
    readonly onMicrophoneToggle: () => void;
    readonly onCameraToggle: () => void;
    readonly onUploadFiles: (files: FileList) => void;
  };

  let {
    viewModel,
    onRenderBackendChange,
    onHiDpiChange,
    onScrollCopyChange,
    onMicrophoneToggle,
    onCameraToggle,
    onUploadFiles,
  }: DisplayControlsPanelProps = $props();
  let fileInput: HTMLInputElement | null = null;

  function renderBackendValue(event: Event): BrowserSessionRenderBackend {
    return (event.currentTarget as HTMLSelectElement).value as BrowserSessionRenderBackend;
  }

  function checkedValue(event: Event): boolean {
    return (event.currentTarget as HTMLInputElement).checked;
  }

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

<section class="grid gap-4" aria-label="Display and media controls">
  <p class="m-0 text-sm font-bold text-admin-ink/68" data-testid="display-connection-label">
    {viewModel.connectionLabel}
  </p>

  <div class="grid gap-3">
    <label class="grid gap-1.5 text-sm font-bold text-admin-ink">
      Render backend
      <select
        class="min-h-10 rounded-[14px] border border-admin-ink/14 bg-admin-cream px-3 text-admin-ink"
        data-testid="display-render-backend"
        value={viewModel.renderBackend}
        onchange={(event) => onRenderBackendChange(renderBackendValue(event))}
      >
        {#each DISPLAY_BACKEND_OPTIONS as option}
          <option value={option.value}>{option.label}</option>
        {/each}
      </select>
    </label>

    <label class="flex items-start gap-3 rounded-[16px] bg-admin-leaf/10 p-3 text-sm font-bold text-admin-ink">
      <input
        class="mt-1"
        type="checkbox"
        data-testid="display-hidpi"
        checked={viewModel.hiDpiEnabled}
        onchange={(event) => onHiDpiChange(checkedValue(event))}
      />
      <span>
        HiDPI rendering
        <small class="block font-normal text-admin-ink/62">Sharper tiles on dense displays.</small>
      </span>
    </label>

    <label class="flex items-start gap-3 rounded-[16px] bg-admin-leaf/10 p-3 text-sm font-bold text-admin-ink">
      <input
        class="mt-1"
        type="checkbox"
        data-testid="display-scroll-copy"
        checked={viewModel.scrollCopyEnabled}
        onchange={(event) => onScrollCopyChange(checkedValue(event))}
      />
      <span>
        Scroll copy
        <small class="block font-normal text-admin-ink/62">Reuse moved pixels during scroll-heavy pages.</small>
      </span>
    </label>
  </div>

  <div class="flex flex-wrap gap-2">
    <button
      class="admin-button-primary"
      type="button"
      data-testid="display-mic-toggle"
      disabled={viewModel.microphoneDisabled}
      onclick={onMicrophoneToggle}
    >
      {viewModel.microphoneLabel}
    </button>
    <button
      class="admin-button-primary"
      type="button"
      data-testid="display-camera-toggle"
      disabled={viewModel.cameraDisabled}
      onclick={onCameraToggle}
    >
      {viewModel.cameraLabel}
    </button>
    <button
      class="admin-button-primary"
      type="button"
      data-testid="display-upload"
      disabled={viewModel.uploadDisabled}
      onclick={chooseFiles}
    >
      Upload files
    </button>
  </div>

  <input
    class="sr-only"
    type="file"
    multiple
    data-testid="display-upload-input"
    bind:this={fileInput}
    onchange={handleUploadChange}
  />

  <p class="m-0 text-sm text-admin-ink/62">{viewModel.reconnectHint}</p>
  {#if viewModel.busy}
    <p class="admin-empty" data-testid="display-busy">Applying display control...</p>
  {/if}
  {#if viewModel.error}
    <p class="admin-error" data-testid="display-error">{viewModel.error}</p>
  {/if}
</section>
