<script lang="ts">
  import {
    PROJECT_POLICY_ALLOW_LIST_GROUPS,
    PROJECT_POLICY_BOOLEAN_FIELDS,
    mergePolicyOptionsWithSelected,
    type ProjectEditDraft,
    type ProjectEditFieldErrors,
    type ProjectPolicyAllowedIdsDraftKey,
    type ProjectPolicyBooleanDraftKey,
    type ProjectPolicyRestrictionDraftKey,
  } from '$lib/projects/project-edit-view-model';
  import type { ProjectPolicyOptionsLoadState } from '$lib/projects/project-detail-state';
  import type { ProjectPolicyOption } from '$lib/projects/project-types';
  import AdminMessage from './AdminMessage.svelte';

  type ProjectPolicyEditorProps = {
    readonly draft: ProjectEditDraft;
    readonly disabled?: boolean;
    readonly policyOptionsState?: ProjectPolicyOptionsLoadState;
    readonly fieldErrors?: ProjectEditFieldErrors;
    readonly onDraftChange?: (draft: ProjectEditDraft) => void;
  };

  let {
    draft,
    disabled = false,
    policyOptionsState = { status: 'idle' },
    fieldErrors = {},
    onDraftChange,
  }: ProjectPolicyEditorProps = $props();

  function setBooleanField(key: ProjectPolicyBooleanDraftKey, value: boolean): void {
    onDraftChange?.({
      ...draft,
      [key]: value,
    });
  }

  function setRestricted(key: ProjectPolicyRestrictionDraftKey, value: boolean): void {
    onDraftChange?.({
      ...draft,
      [key]: value,
    });
  }

  function setSelectedId(key: ProjectPolicyAllowedIdsDraftKey, id: string, selected: boolean): void {
    const current = draft[key];
    const next = selected
      ? [...current, id]
      : current.filter((entry) => entry !== id);
    onDraftChange?.({
      ...draft,
      [key]: [...new Set(next)].sort((left, right) => left.localeCompare(right)),
    });
  }

  function selectedIds(key: ProjectPolicyAllowedIdsDraftKey): readonly string[] {
    return draft[key];
  }

  function restricted(key: ProjectPolicyRestrictionDraftKey): boolean {
    return draft[key];
  }

  function optionsFor(group: (typeof PROJECT_POLICY_ALLOW_LIST_GROUPS)[number]): readonly ProjectPolicyOption[] {
    const options = policyOptionsState.status === 'ready' ? policyOptionsState.options[group.optionsKey] : [];
    return mergePolicyOptionsWithSelected(options, selectedIds(group.selectedIdsKey));
  }

  function inputChecked(event: Event): boolean {
    return (event.currentTarget as HTMLInputElement).checked;
  }

  function selectValue(event: Event): ProjectEditDraft['usageBudgetEnforcement'] {
    return (event.currentTarget as HTMLSelectElement).value as ProjectEditDraft['usageBudgetEnforcement'];
  }
</script>

<section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="project-policy-editor">
  <div class="border-b border-admin-border pb-3">
    <h4 class="m-0 text-sm font-semibold text-admin-ink">Policy</h4>
    <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
      Project policy controls live file capabilities, budget enforcement, and allowed resource sets.
    </p>
  </div>

  <div class="mt-4 grid gap-3 lg:grid-cols-2">
    {#each PROJECT_POLICY_BOOLEAN_FIELDS as field}
      <label
        class={`flex items-start gap-3 rounded-md border p-3 text-sm transition ${
          draft[field.key]
            ? 'border-admin-accent/50 bg-white shadow-sm'
            : 'border-admin-border bg-admin-panel'
        }`}
      >
        <input
          class="mt-1 h-4 w-4 rounded border-admin-border text-admin-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 disabled:cursor-not-allowed disabled:opacity-60"
          type="checkbox"
          checked={draft[field.key]}
          disabled={disabled}
          onchange={(event) => setBooleanField(field.key, inputChecked(event))}
          data-testid={`project-policy-${field.testId}`}
        />
        <span class="min-w-0">
          <span class="block font-medium text-admin-ink">{field.label}</span>
          <span class="mt-0.5 block text-xs leading-5 text-admin-muted">{field.description}</span>
        </span>
      </label>
    {/each}
  </div>

  <label class="mt-4 grid gap-1 text-sm">
    <span class="font-medium text-admin-ink">Usage budget enforcement</span>
    <select
      class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
      value={draft.usageBudgetEnforcement}
      disabled={disabled}
      onchange={(event) => onDraftChange?.({ ...draft, usageBudgetEnforcement: selectValue(event) })}
      data-testid="project-policy-budget-enforcement"
    >
      <option value="warning_only">warning_only</option>
      <option value="block_session_creation">block_session_creation</option>
    </select>
    <span class="text-xs text-admin-muted">
      Blocking enforcement rejects new sessions after selected session/runtime budgets are exhausted.
    </span>
    {#if fieldErrors.usageBudgetEnforcement?.length}
      <AdminMessage
        tone="error"
        density="compact"
        items={fieldErrors.usageBudgetEnforcement}
        testId="project-policy-budget-enforcement-error"
      />
    {/if}
  </label>

  <div class="mt-5 grid gap-4" data-testid="project-policy-allow-lists">
    {#if policyOptionsState.status === 'loading'}
      <AdminMessage
        tone="loading"
        title="Loading selectable resources"
        message="Session templates, browser contexts, and related project resources are being refreshed."
        testId="project-policy-options-loading"
      />
    {:else if policyOptionsState.status === 'error'}
      <AdminMessage
        tone="warning"
        title="Selectable resources could not be refreshed"
        message={policyOptionsState.message}
        testId="project-policy-options-error"
      />
    {/if}

    {#each PROJECT_POLICY_ALLOW_LIST_GROUPS as group}
      <section
        class={`min-w-0 rounded-md border p-3 transition ${
          restricted(group.restrictedKey)
            ? 'border-admin-accent/50 bg-white shadow-sm'
            : 'border-admin-border bg-admin-panel'
        }`}
        data-testid={`project-policy-${group.testId}`}
      >
        <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div class="min-w-0">
            <h5 class="m-0 text-sm font-semibold text-admin-ink">{group.label}</h5>
            <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">{group.description}</p>
          </div>
          <label class="inline-flex shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-2.5 py-1.5 text-sm font-medium text-admin-ink">
            <input
              class="h-4 w-4 rounded border-admin-border text-admin-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 disabled:cursor-not-allowed disabled:opacity-60"
              type="checkbox"
              checked={restricted(group.restrictedKey)}
              disabled={disabled}
              onchange={(event) => setRestricted(group.restrictedKey, inputChecked(event))}
              data-testid={`project-policy-${group.testId}-restrict`}
            />
            <span>Restrict</span>
          </label>
        </div>

        {#if !restricted(group.restrictedKey)}
          <p class="m-0 mt-3 rounded-md bg-admin-soft px-3 py-2 text-xs text-admin-muted" data-testid={`project-policy-${group.testId}-unrestricted`}>
            Unrestricted. The API will persist an empty allow-list for this resource type.
          </p>
        {:else}
          <div class="mt-3 max-h-48 overflow-y-auto rounded-md border border-admin-border bg-admin-panel" data-testid={`project-policy-${group.testId}-options`}>
            {#if optionsFor(group).length === 0}
              <p class="m-0 px-3 py-3 text-sm text-admin-muted">
                No selectable resources are available yet.
              </p>
            {:else}
              {#each optionsFor(group) as option}
                <label
                  class={`flex items-start gap-3 border-b border-admin-border px-3 py-2 text-sm transition last:border-b-0 ${
                    selectedIds(group.selectedIdsKey).includes(option.id)
                      ? 'bg-admin-soft'
                      : 'bg-admin-panel hover:bg-admin-soft/70'
                  }`}
                >
                  <input
                    class="mt-1 h-4 w-4 rounded border-admin-border text-admin-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 disabled:cursor-not-allowed disabled:opacity-60"
                    type="checkbox"
                    checked={selectedIds(group.selectedIdsKey).includes(option.id)}
                    disabled={disabled}
                    onchange={(event) => setSelectedId(group.selectedIdsKey, option.id, inputChecked(event))}
                    data-testid={`project-policy-${group.testId}-option`}
                    data-option-id={option.id}
                  />
                  <span class="min-w-0">
                    <span class="block font-medium text-admin-ink">{option.name}</span>
                    <span class="mt-0.5 block break-words text-xs leading-5 text-admin-muted">
                      {option.description ?? option.id}
                      {#if option.state}
                        <span> · {option.state}</span>
                      {/if}
                      {#if option.scope}
                        <span> · {option.scope}</span>
                      {/if}
                    </span>
                  </span>
                </label>
              {/each}
            {/if}
          </div>
        {/if}

        {#if fieldErrors[group.selectedIdsKey]?.length}
          <div class="mt-3">
            <AdminMessage
              tone="error"
              density="compact"
              items={fieldErrors[group.selectedIdsKey]}
              testId={`project-policy-${group.testId}-error`}
            />
          </div>
        {/if}
      </section>
    {/each}
  </div>
</section>
