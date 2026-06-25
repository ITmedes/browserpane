<script lang="ts">
  import {
    PROJECT_QUOTA_LIMITS,
    PROJECT_ROLLING_SESSION_CREATION_QUOTA,
    type ProjectEditDraft,
    type ProjectEditFieldErrors,
    type ProjectQuotaLimitDraftKey,
    type ProjectQuotaPresetDefinition,
    type ProjectRollingSessionCreationQuotaPresetDefinition,
  } from '$lib/projects/project-edit-view-model';
  import AdminMessage from './AdminMessage.svelte';

  type ProjectQuotaEditorProps = {
    readonly draft: ProjectEditDraft;
    readonly disabled?: boolean;
    readonly fieldErrors?: ProjectEditFieldErrors;
    readonly onDraftChange?: (draft: ProjectEditDraft) => void;
  };

  let {
    draft,
    disabled = false,
    fieldErrors = {},
    onDraftChange,
  }: ProjectQuotaEditorProps = $props();

  const rollingQuotaErrors = $derived(uniqueMessages([
    ...(fieldErrors.maxSessionCreationsPerWindow ?? []),
    ...(fieldErrors.sessionCreationWindowSec ?? []),
  ]));
  const rollingQuotaEnabled = $derived(
    draft.maxSessionCreationsPerWindow.enabled || draft.sessionCreationWindowSec.enabled,
  );

  function setQuotaEnabled(key: ProjectQuotaLimitDraftKey, enabled: boolean): void {
    onDraftChange?.({
      ...draft,
      [key]: {
        ...draft[key],
        enabled,
      },
    });
  }

  function setRollingSessionCreationEnabled(enabled: boolean): void {
    onDraftChange?.({
      ...draft,
      maxSessionCreationsPerWindow: {
        ...draft.maxSessionCreationsPerWindow,
        enabled,
      },
      sessionCreationWindowSec: {
        ...draft.sessionCreationWindowSec,
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

  function applyQuotaPreset(key: ProjectQuotaLimitDraftKey, preset: ProjectQuotaPresetDefinition): void {
    onDraftChange?.({
      ...draft,
      [key]: {
        ...draft[key],
        enabled: true,
        value: preset.value,
      },
    });
  }

  function applyQuotaSliderValue(
    key: ProjectQuotaLimitDraftKey,
    presets: readonly ProjectQuotaPresetDefinition[],
    value: string,
  ): void {
    applyQuotaPreset(key, presetAt(presets, value));
  }

  function applyRollingSessionCreationPreset(preset: ProjectRollingSessionCreationQuotaPresetDefinition): void {
    onDraftChange?.({
      ...draft,
      maxSessionCreationsPerWindow: {
        ...draft.maxSessionCreationsPerWindow,
        enabled: true,
        value: preset.limitValue,
      },
      sessionCreationWindowSec: {
        ...draft.sessionCreationWindowSec,
        enabled: true,
        value: preset.windowValue,
      },
    });
  }

  function applyRollingSessionCreationSliderValue(value: string): void {
    applyRollingSessionCreationPreset(presetAt(PROJECT_ROLLING_SESSION_CREATION_QUOTA.presets, value));
  }

  function quotaSliderIndex(key: ProjectQuotaLimitDraftKey, presets: readonly ProjectQuotaPresetDefinition[]): number {
    if (!draft[key].enabled) {
      return 0;
    }
    const exactIndex = presets.findIndex((preset) => preset.value === draft[key].value);
    return exactIndex >= 0 ? exactIndex : nearestQuotaPresetIndex(draft[key].value, presets);
  }

  function quotaSliderLabel(key: ProjectQuotaLimitDraftKey, presets: readonly ProjectQuotaPresetDefinition[]): string {
    if (!draft[key].enabled) {
      return 'Unbounded';
    }
    const exactMatch = presets.find((preset) => preset.value === draft[key].value);
    return exactMatch ? exactMatch.label : `Custom ${draft[key].value || '--'}`;
  }

  function rollingSliderIndex(): number {
    if (!rollingQuotaEnabled) {
      return 0;
    }
    const exactIndex = PROJECT_ROLLING_SESSION_CREATION_QUOTA.presets.findIndex((preset) =>
      preset.limitValue === draft.maxSessionCreationsPerWindow.value
      && preset.windowValue === draft.sessionCreationWindowSec.value
    );
    return exactIndex >= 0 ? exactIndex : 0;
  }

  function rollingSliderLabel(): string {
    if (!rollingQuotaEnabled) {
      return 'Unbounded';
    }
    const exactMatch = PROJECT_ROLLING_SESSION_CREATION_QUOTA.presets.find((preset) =>
      preset.limitValue === draft.maxSessionCreationsPerWindow.value
      && preset.windowValue === draft.sessionCreationWindowSec.value
    );
    if (exactMatch) {
      return exactMatch.label;
    }
    const limit = draft.maxSessionCreationsPerWindow.value || '--';
    const window = draft.sessionCreationWindowSec.value || '--';
    return `Custom ${limit} / ${window}s`;
  }

  function nearestQuotaPresetIndex(value: string, presets: readonly ProjectQuotaPresetDefinition[]): number {
    const parsed = Number(value);
    if (!Number.isFinite(parsed)) {
      return 0;
    }
    return presets.reduce((bestIndex, preset, index) => {
      const currentDistance = Math.abs(Number(preset.value) - parsed);
      const bestDistance = Math.abs(Number(presets[bestIndex].value) - parsed);
      return currentDistance < bestDistance ? index : bestIndex;
    }, 0);
  }

  function presetAt<T>(presets: readonly T[], value: string): T {
    const index = Number(value);
    if (!Number.isInteger(index) || index < 0 || index >= presets.length) {
      return presets[0];
    }
    return presets[index];
  }

  function inputChecked(event: Event): boolean {
    return (event.currentTarget as HTMLInputElement).checked;
  }

  function inputValue(event: Event): string {
    return (event.currentTarget as HTMLInputElement).value;
  }

  function uniqueMessages(messages: readonly string[]): readonly string[] {
    return [...new Set(messages)];
  }
</script>

<section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="project-quota-editor">
  <div class="border-b border-admin-border pb-3">
    <h4 class="m-0 text-sm font-semibold text-admin-ink">Quotas and usage alerts</h4>
    <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
      Enable the limits this project should enforce or monitor. Leave a quota disabled to persist it as unbounded.
    </p>
  </div>

  <div class="mt-4 grid gap-3 lg:grid-cols-2">
    {#each PROJECT_QUOTA_LIMITS as quota}
      <div
        class={`min-w-0 rounded-md border p-3 transition ${
          draft[quota.key].enabled
            ? 'border-admin-accent/50 bg-white shadow-sm'
            : 'border-admin-border bg-admin-panel'
        }`}
        data-testid={`project-quota-${quota.testId}`}
      >
        <label class="flex items-start gap-3 text-sm">
          <input
            class="mt-1 h-4 w-4 rounded border-admin-border text-admin-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 disabled:cursor-not-allowed disabled:opacity-60"
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

        <label class="mt-3 grid gap-1.5 text-sm">
          <span class="text-xs font-semibold uppercase text-admin-muted">{quota.unitLabel}</span>
          <input
            class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="number"
            min="1"
            step="1"
            inputmode="numeric"
            value={draft[quota.key].value}
            disabled={disabled || !draft[quota.key].enabled}
            oninput={(event) => setQuotaValue(quota.key, inputValue(event))}
            data-testid={`project-quota-${quota.testId}-value`}
          />
          {#if fieldErrors[quota.key]?.length}
            <AdminMessage
              tone="error"
              density="compact"
              items={fieldErrors[quota.key]}
              testId={`project-quota-${quota.testId}-error`}
            />
          {/if}
        </label>

        {#if quota.presets?.length}
          <div class="mt-3 grid gap-2" aria-label={`${quota.label} presets`}>
            <div class="flex min-w-0 items-center justify-between gap-3">
              <span class="text-xs font-semibold uppercase text-admin-muted">Recommended</span>
              <span
                class="truncate rounded-md bg-admin-soft px-2 py-1 text-xs font-medium text-admin-ink"
                data-testid={`project-quota-${quota.testId}-slider-label`}
              >
                {quotaSliderLabel(quota.key, quota.presets)}
              </span>
            </div>
            <input
              class="h-2 w-full cursor-pointer rounded-lg bg-admin-border accent-admin-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60"
              type="range"
              min="0"
              max={quota.presets.length - 1}
              step="1"
              value={quotaSliderIndex(quota.key, quota.presets)}
              disabled={disabled}
              oninput={(event) => applyQuotaSliderValue(quota.key, quota.presets, inputValue(event))}
              data-testid={`project-quota-${quota.testId}-slider`}
            />
            <div class="grid auto-cols-fr grid-flow-col gap-1 text-[10px] font-medium text-admin-muted">
              {#each quota.presets as preset}
                <span class="min-w-0 truncate text-center">{preset.label}</span>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    {/each}

    <div
      class={`min-w-0 rounded-md border p-3 transition ${
        rollingQuotaEnabled
          ? 'border-admin-accent/50 bg-white shadow-sm'
          : 'border-admin-border bg-admin-panel'
      }`}
      data-testid={`project-quota-${PROJECT_ROLLING_SESSION_CREATION_QUOTA.testId}`}
    >
      <label class="flex items-start gap-3 text-sm">
        <input
          class="mt-1 h-4 w-4 rounded border-admin-border text-admin-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 disabled:cursor-not-allowed disabled:opacity-60"
          type="checkbox"
          checked={rollingQuotaEnabled}
          disabled={disabled}
          onchange={(event) => setRollingSessionCreationEnabled(inputChecked(event))}
          data-testid={`project-quota-${PROJECT_ROLLING_SESSION_CREATION_QUOTA.testId}-enabled`}
        />
        <span class="min-w-0">
          <span class="block font-medium text-admin-ink">Limit {PROJECT_ROLLING_SESSION_CREATION_QUOTA.label}</span>
          <span class="mt-0.5 block text-xs leading-5 text-admin-muted">
            {PROJECT_ROLLING_SESSION_CREATION_QUOTA.description}
          </span>
        </span>
      </label>

      <div class="mt-3 grid gap-3 sm:grid-cols-2">
        <label class="grid gap-1.5 text-sm">
          <span class="text-xs font-semibold uppercase text-admin-muted">
            {PROJECT_ROLLING_SESSION_CREATION_QUOTA.limitLabel}
          </span>
          <input
            class="h-10 min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="number"
            min="1"
            step="1"
            inputmode="numeric"
            value={draft.maxSessionCreationsPerWindow.value}
            disabled={disabled || !rollingQuotaEnabled}
            oninput={(event) => setQuotaValue('maxSessionCreationsPerWindow', inputValue(event))}
            data-testid="project-quota-max-session-creations-per-window-value"
          />
        </label>

        <label class="grid gap-1.5 text-sm">
          <span class="text-xs font-semibold uppercase text-admin-muted">
            {PROJECT_ROLLING_SESSION_CREATION_QUOTA.windowLabel}
          </span>
          <input
            class="h-10 min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="number"
            min="1"
            step="1"
            inputmode="numeric"
            value={draft.sessionCreationWindowSec.value}
            disabled={disabled || !rollingQuotaEnabled}
            oninput={(event) => setQuotaValue('sessionCreationWindowSec', inputValue(event))}
            data-testid="project-quota-session-creation-window-sec-value"
          />
        </label>
      </div>

      <div class="mt-3 grid gap-2" aria-label="Session creation rate presets">
        <div class="flex min-w-0 items-center justify-between gap-3">
          <span class="text-xs font-semibold uppercase text-admin-muted">Recommended</span>
          <span
            class="truncate rounded-md bg-admin-soft px-2 py-1 text-xs font-medium text-admin-ink"
            data-testid={`project-quota-${PROJECT_ROLLING_SESSION_CREATION_QUOTA.testId}-slider-label`}
          >
            {rollingSliderLabel()}
          </span>
        </div>
        <input
          class="h-2 w-full cursor-pointer rounded-lg bg-admin-border accent-admin-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60"
          type="range"
          min="0"
          max={PROJECT_ROLLING_SESSION_CREATION_QUOTA.presets.length - 1}
          step="1"
          value={rollingSliderIndex()}
          disabled={disabled}
          oninput={(event) => applyRollingSessionCreationSliderValue(inputValue(event))}
          data-testid={`project-quota-${PROJECT_ROLLING_SESSION_CREATION_QUOTA.testId}-slider`}
        />
        <div class="grid auto-cols-fr grid-flow-col gap-1 text-[10px] font-medium text-admin-muted">
          {#each PROJECT_ROLLING_SESSION_CREATION_QUOTA.presets as preset}
            <span class="min-w-0 truncate text-center">{preset.label}</span>
          {/each}
        </div>
      </div>

      {#if rollingQuotaErrors.length}
        <div class="mt-3">
          <AdminMessage
            tone="error"
            density="compact"
            items={rollingQuotaErrors}
            testId={`project-quota-${PROJECT_ROLLING_SESSION_CREATION_QUOTA.testId}-error`}
          />
        </div>
      {/if}
    </div>
  </div>
</section>
