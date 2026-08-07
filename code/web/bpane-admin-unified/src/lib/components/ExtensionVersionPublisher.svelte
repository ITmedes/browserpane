<script lang="ts">
  import {
    createExtensionVersionDraft,
    validateExtensionVersionDraft,
    type ExtensionVersionDraft,
  } from '$lib/extensions/extension-view-model';
  import type { CreateExtensionVersionRequest } from '$lib/extensions/extension-types';
  import AdminMessage from './AdminMessage.svelte';

  let {
    disabled = false,
    onPublish,
  }: {
    readonly disabled?: boolean;
    readonly onPublish?: (request: CreateExtensionVersionRequest) => void | Promise<void>;
  } = $props();
  let draft = $state<ExtensionVersionDraft>(createExtensionVersionDraft());
  const validation = $derived(validateExtensionVersionDraft(draft));

  function publish(): void {
    if (validation.request) {
      void onPublish?.(validation.request);
    }
  }
</script>

<section
  class="rounded-md border border-admin-border bg-admin-soft/50 p-4"
  data-testid="extension-version-form"
>
  <div class="border-b border-admin-border pb-3">
    <h3 class="m-0 text-sm font-semibold text-admin-ink">Publish installed version</h3>
    <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
      The path must be absolute and already available inside supported docker-backed browser
      runtimes. BrowserPane does not upload extension packages here.
    </p>
  </div>
  <form
    class="mt-4 grid gap-4 lg:grid-cols-[220px_minmax(0,1fr)_auto] lg:items-start"
    onsubmit={(event) => {
      event.preventDefault();
      publish();
    }}
  >
    <label class="grid gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Version</span>
      <input
        class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent"
        type="text"
        bind:value={draft.version}
        {disabled}
        placeholder="1.0.0"
        data-testid="extension-version-value"
      />
      {#if validation.fieldErrors.version}
        <AdminMessage
          tone="error"
          density="compact"
          items={validation.fieldErrors.version}
          testId="extension-version-value-error"
        />
      {/if}
    </label>
    <label class="grid gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Installed path</span>
      <input
        class="h-10 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent"
        type="text"
        bind:value={draft.installPath}
        {disabled}
        placeholder="/opt/browserpane/extensions/example"
        data-testid="extension-version-path"
      />
      {#if validation.fieldErrors.installPath}
        <AdminMessage
          tone="error"
          density="compact"
          items={validation.fieldErrors.installPath}
          testId="extension-version-path-error"
        />
      {/if}
    </label>
    <button
      class="mt-[26px] inline-flex h-10 items-center justify-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white disabled:opacity-60"
      type="submit"
      disabled={disabled || !validation.valid}
      data-testid="extension-version-submit">Publish version</button
    >
  </form>
</section>
