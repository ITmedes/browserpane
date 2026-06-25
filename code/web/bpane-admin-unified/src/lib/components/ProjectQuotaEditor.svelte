<script lang="ts">
  import {
    PROJECT_QUOTA_LIMITS,
    type ProjectEditDraft,
    type ProjectQuotaLimitDraftKey,
  } from '$lib/projects/project-edit-view-model';

  type ProjectQuotaEditorProps = {
    readonly draft: ProjectEditDraft;
    readonly disabled?: boolean;
    readonly onDraftChange?: (draft: ProjectEditDraft) => void;
  };

  let {
    draft,
    disabled = false,
    onDraftChange,
  }: ProjectQuotaEditorProps = $props();

  function setQuotaEnabled(key: ProjectQuotaLimitDraftKey, enabled: boolean): void {
    onDraftChange?.({
      ...draft,
      [key]: {
        ...draft[key],
        enabled,
      },
    });
  }

  function setQuotaValue(key: ProjectQuotaLimitDraftKey, value: string): void {
    onDraftChange?.({
      ...draft,
      [key]: {
        ...draft[key],
        value,
      },
    });
  }

  function inputChecked(event: Event): boolean {
    return (event.currentTarget as HTMLInputElement).checked;
  }

  function inputValue(event: Event): string {
    return (event.currentTarget as HTMLInputElement).value;
  }
</script>

<section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="project-quota-editor">
  <div class="border-b border-admin-border pb-3">
    <h4 class="m-0 text-sm font-semibold text-admin-ink">Quotas and usage alerts</h4>
    <p class="m-0 mt-1 text-xs text-admin-muted">
      Enable a quota to persist a positive integer limit. Usage alerts are generated from these limits by the backend.
    </p>
  </div>

  <div class="mt-4 grid gap-3 lg:grid-cols-2">
    {#each PROJECT_QUOTA_LIMITS as quota}
      <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3" data-testid={`project-quota-${quota.testId}`}>
        <label class="flex items-start gap-3 text-sm">
          <input
            class="mt-1 h-4 w-4 rounded border-admin-border text-admin-accent focus:ring-admin-accent"
            type="checkbox"
            checked={draft[quota.key].enabled}
            disabled={disabled}
            onchange={(event) => setQuotaEnabled(quota.key, inputChecked(event))}
            data-testid={`project-quota-${quota.testId}-enabled`}
          />
          <span class="min-w-0">
            <span class="block font-medium text-admin-ink">Limit {quota.label}</span>
            <span class="mt-0.5 block text-xs leading-5 text-admin-muted">{quota.description}</span>
          </span>
        </label>

        <label class="mt-3 grid gap-1 text-sm">
          <span class="text-xs font-semibold uppercase text-admin-muted">{quota.unitLabel}</span>
          <input
            class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="number"
            min="1"
            step="1"
            inputmode="numeric"
            value={draft[quota.key].value}
            disabled={disabled || !draft[quota.key].enabled}
            oninput={(event) => setQuotaValue(quota.key, inputValue(event))}
            data-testid={`project-quota-${quota.testId}-value`}
          />
        </label>
      </div>
    {/each}
  </div>

  <p class="m-0 mt-3 text-xs text-admin-muted" data-testid="project-usage-alert-note">
    Alert thresholds are not editable here. The project API reports generated alerts when session, runtime, or egress budgets approach or exceed their limits.
  </p>
</section>
