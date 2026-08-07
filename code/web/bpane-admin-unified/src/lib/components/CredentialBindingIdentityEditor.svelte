<script lang="ts">
  import type {
    CredentialBindingDraft,
    CredentialBindingValidation,
  } from '$lib/credential-bindings/credential-binding-form-model';
  import type { CredentialBindingProjectOptionsLoadState } from '$lib/credential-bindings/credential-binding-view-model';
  import AdminMessage from './AdminMessage.svelte';

  let {
    draft = $bindable(),
    validation,
    projectOptionsState = { status: 'idle' },
    disabled = false,
  }: {
    draft: CredentialBindingDraft;
    readonly validation: CredentialBindingValidation;
    readonly projectOptionsState?: CredentialBindingProjectOptionsLoadState;
    readonly disabled?: boolean;
  } = $props();
</script>

<section class="grid gap-4 rounded-md border border-admin-border bg-admin-soft/50 p-4">
  <div>
    <h3 class="m-0 text-sm font-semibold text-admin-ink">Identity and scope</h3>
    <p class="m-0 mt-1 text-xs text-admin-muted">
      Project scope restricts which sessions and workflows may consume this binding.
    </p>
  </div>
  <label class="grid gap-1.5 text-sm"
    ><span class="font-medium text-admin-ink">Name</span><input
      class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent"
      type="text"
      bind:value={draft.name}
      {disabled}
      data-testid="credential-binding-name"
    />{#if validation.fieldErrors.name}<AdminMessage
        tone="error"
        density="compact"
        items={validation.fieldErrors.name}
        testId="credential-binding-name-error"
      />{/if}</label
  >
  <fieldset class="grid gap-2">
    <legend class="text-sm font-medium text-admin-ink">Scope</legend>
    <div class="inline-flex w-fit rounded-md border border-admin-border bg-white p-1">
      {#each ['owner', 'project'] as scope}<label
          class={`cursor-pointer rounded px-3 py-1.5 text-sm ${draft.scope === scope ? 'bg-admin-accent text-white' : 'text-admin-muted'}`}
          ><input
            class="sr-only"
            type="radio"
            name="binding-scope"
            value={scope}
            bind:group={draft.scope}
            {disabled}
            data-testid={`credential-binding-scope-${scope}`}
          />{scope === 'owner' ? 'Owner' : 'Project'}</label
        >{/each}
    </div>
  </fieldset>
  {#if draft.scope === 'project'}
    <label class="grid gap-1.5 text-sm"
      ><span class="font-medium text-admin-ink">Project</span><select
        class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink"
        bind:value={draft.projectId}
        disabled={disabled || projectOptionsState.status !== 'ready'}
        data-testid="credential-binding-project"
        ><option value="">Select project</option
        >{#if projectOptionsState.status === 'ready'}{#each projectOptionsState.projects.filter((project) => project.state === 'active') as project}<option
              value={project.id}>{project.name}</option
            >{/each}{/if}</select
      >{#if validation.fieldErrors.projectId}<AdminMessage
          tone="error"
          density="compact"
          items={validation.fieldErrors.projectId}
          testId="credential-binding-project-error"
        />{/if}{#if projectOptionsState.status === 'error'}<AdminMessage
          tone="error"
          density="compact"
          title="Project options unavailable"
          message={projectOptionsState.message}
          testId="credential-binding-projects-error"
        />{/if}</label
    >
  {/if}
  <label class="grid gap-1.5 text-sm"
    ><span class="font-medium text-admin-ink">Namespace</span><input
      class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink"
      type="text"
      bind:value={draft.namespace}
      {disabled}
      placeholder="Optional provider namespace"
      data-testid="credential-binding-namespace"
    /></label
  >
  <label class="grid gap-1.5 text-sm"
    ><span class="font-medium text-admin-ink">Labels</span><textarea
      class="min-h-20 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs text-admin-ink"
      bind:value={draft.labelsText}
      {disabled}
      placeholder="team=support"
      data-testid="credential-binding-labels"
    ></textarea>{#if validation.fieldErrors.labels}<AdminMessage
        tone="error"
        density="compact"
        items={validation.fieldErrors.labels}
        testId="credential-binding-labels-error"
      />{/if}</label
  >
</section>
