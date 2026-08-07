<script lang="ts">
  import type {
    CredentialBindingDraft,
    CredentialBindingValidation,
  } from '$lib/credential-bindings/credential-binding-form-model';
  import { createCredentialBindingDraft } from '$lib/credential-bindings/credential-binding-form-model';
  import type { CredentialBindingProjectOptionsLoadState } from '$lib/credential-bindings/credential-binding-view-model';
  import CredentialBindingIdentityEditor from '$lib/components/CredentialBindingIdentityEditor.svelte';
  import CredentialBindingInjectionEditor from '$lib/components/CredentialBindingInjectionEditor.svelte';
  import CredentialBindingSecretEditor from '$lib/components/CredentialBindingSecretEditor.svelte';

  let {
    editor,
    initialDraft,
    validation,
    projectOptionsState = { status: 'idle' },
  }: {
    readonly editor: 'identity' | 'injection' | 'secret';
    readonly initialDraft: CredentialBindingDraft;
    readonly validation: CredentialBindingValidation;
    readonly projectOptionsState?: CredentialBindingProjectOptionsLoadState;
  } = $props();
  let draft = $state<CredentialBindingDraft>(createCredentialBindingDraft());
  $effect.pre(() => {
    Object.assign(draft, initialDraft);
  });
</script>

{#if editor === 'identity'}
  <CredentialBindingIdentityEditor bind:draft {validation} {projectOptionsState} />
{:else if editor === 'injection'}
  <CredentialBindingInjectionEditor bind:draft {validation} />
{:else}
  <CredentialBindingSecretEditor bind:draft {validation} />
{/if}
