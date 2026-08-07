<script lang="ts">
  import type {
    CredentialBindingDraft,
    CredentialBindingValidation,
  } from '$lib/credential-bindings/credential-binding-form-model';
  import AdminMessage from './AdminMessage.svelte';

  let {
    draft = $bindable(),
    validation,
    disabled = false,
  }: {
    draft: CredentialBindingDraft;
    readonly validation: CredentialBindingValidation;
    readonly disabled?: boolean;
  } = $props();
</script>

<section class="grid gap-4 rounded-md border border-admin-border bg-admin-soft/50 p-4">
  <div>
    <h3 class="m-0 text-sm font-semibold text-admin-ink">Write-only secret source</h3>
    <p class="m-0 mt-1 text-xs text-admin-muted">
      Provide either a new JSON payload or an opaque reference that already exists in Vault.
    </p>
  </div>
  <fieldset class="grid gap-2">
    <legend class="text-sm font-medium text-admin-ink">Source</legend>
    <div class="inline-flex w-fit rounded-md border border-admin-border bg-white p-1">
      {#each [{ value: 'payload', label: 'New payload' }, { value: 'external_ref', label: 'Existing reference' }] as source}<label
          class={`cursor-pointer rounded px-3 py-1.5 text-sm ${draft.secretSource === source.value ? 'bg-admin-accent text-white' : 'text-admin-muted'}`}
          ><input
            class="sr-only"
            type="radio"
            name="secret-source"
            value={source.value}
            bind:group={draft.secretSource}
            {disabled}
            data-testid={`credential-binding-secret-${source.value === 'payload' ? 'payload-mode' : 'reference-mode'}`}
          />{source.label}</label
        >{/each}
    </div>
  </fieldset>
  {#if draft.secretSource === 'payload'}
    <label class="grid gap-1.5 text-sm"
      ><span class="font-medium text-admin-ink">Secret JSON payload</span><textarea
        class="min-h-44 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs leading-5 text-admin-ink"
        bind:value={draft.secretPayloadText}
        {disabled}
        spellcheck="false"
        autocomplete="off"
        data-testid="credential-binding-secret-payload"
      ></textarea><AdminMessage
        tone="warning"
        density="compact"
        title="Write-only value"
        message="BrowserPane sends this value to Vault during creation. It will not appear in catalog or detail responses."
      />{#if validation.fieldErrors.secretPayload}<AdminMessage
          tone="error"
          density="compact"
          items={validation.fieldErrors.secretPayload}
          testId="credential-binding-secret-payload-error"
        />{/if}</label
    >
  {:else}
    <label class="grid gap-1.5 text-sm"
      ><span class="font-medium text-admin-ink">Existing provider reference</span><input
        class="h-10 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink"
        type="text"
        bind:value={draft.externalRef}
        {disabled}
        autocomplete="off"
        data-testid="credential-binding-external-ref"
      />{#if validation.fieldErrors.externalRef}<AdminMessage
          tone="error"
          density="compact"
          items={validation.fieldErrors.externalRef}
          testId="credential-binding-external-ref-error"
        />{/if}</label
    >
  {/if}
</section>
