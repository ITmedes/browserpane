<script lang="ts">
  import { Copy, RefreshCw } from '@lucide/svelte';
  import type {
    EgressProfileActionState,
    EgressProfileDetailLoadState,
    EgressProfileProjectOptionsLoadState,
  } from '$lib/egress-profiles/egress-profile-detail-state';
  import type { UpsertEgressProfileRequest } from '$lib/egress-profiles/egress-profile-types';
  import { egressProfileRow } from '$lib/egress-profiles/egress-profile-overview-view-model';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';
  import ActionFeedback from './ActionFeedback.svelte';
  import EgressProfileEditForm from './EgressProfileEditForm.svelte';

  type EgressProfileInspectorProps = {
    readonly state: EgressProfileDetailLoadState;
    readonly actionState?: EgressProfileActionState;
    readonly projectOptionsState?: EgressProfileProjectOptionsLoadState;
    readonly onRefreshProfile?: () => void | Promise<void>;
    readonly onSaveProfile?: (request: UpsertEgressProfileRequest) => void | Promise<void>;
  };

  let {
    state,
    actionState = { status: 'idle' },
    projectOptionsState = { status: 'idle' },
    onRefreshProfile,
    onSaveProfile,
  }: EgressProfileInspectorProps = $props();

  const row = $derived(state.status === 'ready' ? egressProfileRow(state.profile) : null);
  const busy = $derived(actionState.status === 'running');

  function refreshProfile(): void {
    void onRefreshProfile?.();
  }

  function saveProfile(request: UpsertEgressProfileRequest): void {
    void onSaveProfile?.(request);
  }

  async function copyProfileId(profileId: string): Promise<void> {
    await navigator.clipboard?.writeText(profileId);
  }
</script>

<aside class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="egress-profile-inspector">
  {#if state.status === 'idle'}
    <div class="flex min-h-64 items-center justify-center p-6 text-center text-sm text-admin-muted" data-testid="egress-profile-inspector-idle">
      Select an egress profile to inspect and edit.
    </div>
  {:else if state.status === 'loading'}
    <div class="flex min-h-64 items-center justify-center p-6 text-sm text-admin-muted" data-testid="egress-profile-inspector-loading">
      Loading egress profile...
    </div>
  {:else if state.status === 'error'}
    <div class="p-4">
      <AdminMessage
        tone="error"
        title="Egress profile unavailable"
        message={state.message}
        testId="egress-profile-inspector-error"
      />
    </div>
  {:else if row}
    <div class="border-b border-admin-border p-4">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div class="min-w-0">
          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <h2 class="m-0 min-w-0 max-w-full truncate text-xl font-semibold text-admin-ink">{row.name}</h2>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.stateTone)}`}>
              {row.state}
            </span>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.kindTone)}`}>
              {row.kindLabel}
            </span>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.healthTone)}`}>
              {row.healthLabel}
            </span>
          </div>
          <p class="m-0 mt-1 text-sm text-admin-muted">{row.description}</p>
          <p class="m-0 mt-2 min-w-0 truncate font-mono text-xs text-admin-muted">{row.id}</p>
        </div>

        <div class="flex shrink-0 flex-wrap gap-2">
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
            type="button"
            onclick={() => void copyProfileId(row.id)}
            data-testid="egress-profile-copy-id"
          >
            <Copy size={15} strokeWidth={1.8} />
            <span>Copy ID</span>
          </button>
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
            type="button"
            onclick={refreshProfile}
            disabled={busy}
            data-testid="egress-profile-refresh-detail"
          >
            <RefreshCw size={15} strokeWidth={1.8} />
            <span>Refresh</span>
          </button>
        </div>
      </div>
    </div>

    <div class="border-b border-admin-border p-4">
      <ActionFeedback
        state={actionState}
        successTitle="Egress profile action completed"
        errorTitle="Egress profile action failed"
        successTestId="egress-profile-action-success"
        errorTestId="egress-profile-action-error"
        runningTestId="egress-profile-action-running"
      />
    </div>

    <div class="grid gap-4 p-4">
      {#key `${state.profile.id}:${state.profile.updated_at}`}
        <EgressProfileEditForm
          profile={state.profile}
          disabled={busy}
          {projectOptionsState}
          onSave={saveProfile}
        />
      {/key}
    </div>
  {/if}
</aside>
