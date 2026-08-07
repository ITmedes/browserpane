<script lang="ts">
  import {
    createExtensionDefinitionDraft,
    validateExtensionDefinitionDraft,
    type ExtensionDefinitionDraft,
  } from '$lib/extensions/extension-view-model';
  import type { CreateExtensionDefinitionRequest } from '$lib/extensions/extension-types';
  import AdminMessage from './AdminMessage.svelte';

  let {
    disabled = false,
    onSave,
  }: {
    readonly disabled?: boolean;
    readonly onSave?: (request: CreateExtensionDefinitionRequest) => void | Promise<void>;
  } = $props();
  let draft = $state<ExtensionDefinitionDraft>(createExtensionDefinitionDraft());
  const validation = $derived(validateExtensionDefinitionDraft(draft));

  function save(): void {
    if (validation.request) void onSave?.(validation.request);
  }
</script>

<section
  class="rounded-md border border-admin-border bg-admin-panel p-4 sm:p-5"
  data-testid="extension-create-form"
>
  <div class="border-b border-admin-border pb-4">
    <h2 class="m-0 text-base font-semibold text-admin-ink">Extension reference</h2>
    <p class="m-0 mt-1 text-sm leading-6 text-admin-muted">
      Register approved metadata first. Publish the deployment-managed installed path from the
      detail route.
    </p>
  </div>
  <form
    class="mt-5 grid gap-4"
    onsubmit={(event) => {
      event.preventDefault();
      save();
    }}
  >
    <label class="grid gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Name</span>
      <input
        class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25"
        type="text"
        bind:value={draft.name}
        {disabled}
        autocomplete="off"
        data-testid="extension-create-name"
      />
      {#if validation.fieldErrors.name}<AdminMessage
          tone="error"
          density="compact"
          items={validation.fieldErrors.name}
          testId="extension-create-name-error"
        />{/if}
    </label>
    <label class="grid gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Description</span>
      <textarea
        class="min-h-24 rounded-md border border-admin-border bg-white px-3 py-2 text-sm text-admin-ink outline-none focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25"
        bind:value={draft.description}
        {disabled}
        data-testid="extension-create-description"
      ></textarea>
    </label>
    <label class="grid gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Labels</span>
      <textarea
        class="min-h-24 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25"
        placeholder="team=platform&#10;purpose=login"
        bind:value={draft.labelsText}
        {disabled}
        spellcheck="false"
        data-testid="extension-create-labels"
      ></textarea>
      <span class="text-xs text-admin-muted">One `key=value` label per line.</span>
      {#if validation.fieldErrors.labels}<AdminMessage
          tone="error"
          density="compact"
          items={validation.fieldErrors.labels}
          testId="extension-create-labels-error"
        />{/if}
    </label>
    <div class="flex justify-end border-t border-admin-border pt-4">
      <button
        class="inline-flex h-10 items-center justify-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white disabled:cursor-not-allowed disabled:opacity-60"
        type="submit"
        disabled={disabled || !validation.valid}
        data-testid="extension-create-submit">Create extension</button
      >
    </div>
  </form>
</section>
