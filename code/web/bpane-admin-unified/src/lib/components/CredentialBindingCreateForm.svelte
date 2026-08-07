<script lang="ts">
  import {
    createCredentialBindingDraft,
    validateCredentialBindingDraft,
    type CredentialBindingDraft,
  } from '$lib/credential-bindings/credential-binding-form-model';
  import type { CredentialBindingProjectOptionsLoadState } from '$lib/credential-bindings/credential-binding-view-model';
  import type { CreateCredentialBindingRequest } from '$lib/credential-bindings/credential-binding-types';
  import CredentialBindingIdentityEditor from './CredentialBindingIdentityEditor.svelte';
  import CredentialBindingInjectionEditor from './CredentialBindingInjectionEditor.svelte';
  import CredentialBindingSecretEditor from './CredentialBindingSecretEditor.svelte';

  let {
    disabled = false,
    projectOptionsState = { status: 'idle' },
    onSave,
  }: {
    readonly disabled?: boolean;
    readonly projectOptionsState?: CredentialBindingProjectOptionsLoadState;
    readonly onSave?: (request: CreateCredentialBindingRequest) => void | Promise<void>;
  } = $props();
  let draft = $state<CredentialBindingDraft>(createCredentialBindingDraft());
  const validation = $derived(validateCredentialBindingDraft(draft));
</script>

<section
  class="rounded-md border border-admin-border bg-admin-panel p-4 sm:p-5"
  data-testid="credential-binding-create-form"
>
  <div class="border-b border-admin-border pb-4">
    <h2 class="m-0 text-base font-semibold text-admin-ink">Credential binding</h2>
    <p class="m-0 mt-1 text-sm leading-6 text-admin-muted">
      Secret values are sent once to the configured provider and cannot be read back through
      BrowserPane.
    </p>
  </div>
  <form
    class="mt-5 grid gap-4"
    onsubmit={(event) => {
      event.preventDefault();
      if (validation.request) void onSave?.(validation.request);
    }}
  >
    <CredentialBindingIdentityEditor bind:draft {validation} {projectOptionsState} {disabled} />
    <CredentialBindingInjectionEditor bind:draft {validation} {disabled} />
    <CredentialBindingSecretEditor bind:draft {validation} {disabled} />
    <div class="flex justify-end border-t border-admin-border pt-4">
      <button
        class="inline-flex h-10 items-center justify-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white disabled:opacity-60"
        type="submit"
        disabled={disabled || !validation.valid}
        data-testid="credential-binding-create-submit">Create binding</button
      >
    </div>
  </form>
</section>
