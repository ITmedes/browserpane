<script lang="ts">
  import { Copy, Network, RefreshCw, ShieldCheck, ShieldOff } from 'lucide-svelte';
  import type {
    CreateEgressProfileCommand,
    EgressProfileResource,
  } from '../api/control-types';
  import AdminMessage from './AdminMessage.svelte';
  import type { AdminMessageFeedback } from './admin-message-types';
  import {
    buildEgressProfileCommand,
    commandFromEgressProfile,
    egressProfileRows,
    emptyEgressProfileForm,
    formFromEgressProfile,
  } from './egress-profile-catalog';

  type EgressProfileCatalogPanelProps = {
    readonly profiles: readonly EgressProfileResource[];
    readonly loading?: boolean;
    readonly error?: string | null;
    readonly onRefresh: () => void;
    readonly onCreateProfile?: (command: CreateEgressProfileCommand) => Promise<EgressProfileResource | void> | EgressProfileResource | void;
    readonly onUpdateProfile?: (profileId: string, command: CreateEgressProfileCommand) => Promise<EgressProfileResource | void> | EgressProfileResource | void;
  };

  let {
    profiles,
    loading = false,
    error = null,
    onRefresh,
    onCreateProfile,
    onUpdateProfile,
  }: EgressProfileCatalogPanelProps = $props();

  let search = $state('');
  let selectedProfileId = $state<string | null>(null);
  let editingProfileId = $state<string | null>(null);
  let form = $state(emptyEgressProfileForm());
  let mutating = $state(false);
  let feedback = $state<AdminMessageFeedback | null>(null);
  const rows = $derived(egressProfileRows(profiles, search));
  const selectedProfile = $derived(profiles.find((profile) => profile.id === selectedProfileId) ?? null);
  const editing = $derived(Boolean(editingProfileId));
  const canSave = $derived(Boolean(onCreateProfile && !editing) || Boolean(onUpdateProfile && editing));
  const disabled = $derived(loading || mutating);

  $effect(() => {
    if (!rows.length) {
      selectedProfileId = null;
    } else if (!rows.some((row) => row.id === selectedProfileId)) {
      selectedProfileId = rows[0]?.id ?? null;
    }
  });

  function selectProfile(profileId: string): void {
    selectedProfileId = profileId;
    feedback = null;
  }

  function resetForm(): void {
    editingProfileId = null;
    form = emptyEgressProfileForm();
    feedback = null;
  }

  function editSelected(): void {
    const profile = selectedProfile;
    if (!profile) {
      return;
    }
    editingProfileId = profile.id;
    form = formFromEgressProfile(profile);
    feedback = null;
  }

  function cloneSelected(): void {
    const profile = selectedProfile;
    if (!profile) {
      return;
    }
    editingProfileId = null;
    form = formFromEgressProfile(profile, { clone: true });
    feedback = {
      variant: 'info',
      title: 'Clone prepared',
      message: 'Review the copied profile payload before saving it as a new profile.',
    };
  }

  async function saveProfile(): Promise<void> {
    const result = buildEgressProfileCommand(form);
    if (!result.ok) {
      feedback = { variant: 'error', title: 'Profile validation failed', message: result.error };
      return;
    }
    if (editingProfileId && !onUpdateProfile) {
      feedback = { variant: 'error', title: 'Profile update unavailable', message: 'This admin view cannot update profiles.' };
      return;
    }
    if (!editingProfileId && !onCreateProfile) {
      feedback = { variant: 'error', title: 'Profile create unavailable', message: 'This admin view cannot create profiles.' };
      return;
    }

    mutating = true;
    feedback = null;
    try {
      const saved = editingProfileId
        ? await onUpdateProfile?.(editingProfileId, result.command)
        : await onCreateProfile?.(result.command);
      if (saved?.id) {
        selectedProfileId = saved.id;
      }
      feedback = {
        variant: 'success',
        title: editingProfileId ? 'Profile updated' : 'Profile created',
        message: saved?.name ? `${saved.name} is available in the egress catalog.` : 'Egress profile saved.',
      };
      editingProfileId = null;
      if (!saved?.id) {
        onRefresh();
      }
    } catch (saveError) {
      feedback = {
        variant: 'error',
        title: 'Profile save failed',
        message: saveError instanceof Error ? saveError.message : 'Could not save egress profile.',
      };
    } finally {
      mutating = false;
    }
  }

  async function disableSelected(): Promise<void> {
    const profile = selectedProfile;
    if (!profile || profile.state === 'disabled' || !onUpdateProfile) {
      return;
    }
    mutating = true;
    feedback = null;
    try {
      const updated = await onUpdateProfile(profile.id, commandFromEgressProfile(profile, 'disabled'));
      if (updated?.id) {
        selectedProfileId = updated.id;
      }
      feedback = {
        variant: 'success',
        title: 'Profile disabled',
        message: `${profile.name} will no longer be offered as a healthy launch choice.`,
      };
    } catch (disableError) {
      feedback = {
        variant: 'error',
        title: 'Profile disable failed',
        message: disableError instanceof Error ? disableError.message : 'Could not disable egress profile.',
      };
    } finally {
      mutating = false;
    }
  }

  function rowClass(state: string, selected: boolean): string {
    if (selected) {
      return 'border-admin-leaf/42 bg-admin-leaf/12 text-admin-ink';
    }
    return state === 'disabled'
      ? 'border-admin-danger/24 bg-admin-danger/8 text-admin-ink/78'
      : 'border-[#90a6cc]/18 bg-admin-field/70 text-admin-ink/82';
  }
</script>

<div class="grid min-w-0 gap-3" data-testid="egress-profile-catalog">
  <section class="grid gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Egress profile catalog controls">
    <div class="flex min-w-0 flex-wrap items-start justify-between gap-3">
      <div class="min-w-0">
        <p class="admin-eyebrow mb-1">Egress profiles</p>
        <h3 class="m-0 text-base font-bold text-admin-ink">Approved outbound paths</h3>
      </div>
      <button
        class="admin-button-primary inline-flex items-center gap-2"
        type="button"
        data-testid="egress-profile-refresh"
        disabled={disabled}
        onclick={onRefresh}
      >
        <RefreshCw class={loading ? 'animate-spin' : ''} size={15} aria-hidden="true" />
        Refresh
      </button>
    </div>

    {#if error}
      <AdminMessage variant="warning" title="Egress catalog unavailable" message={error} compact={true} />
    {/if}
    {#if feedback}
      <AdminMessage variant={feedback.variant} title={feedback.title} message={feedback.message} compact={true} onDismiss={() => { feedback = null; }} />
    {/if}

    <div class="grid min-w-0 gap-3 md:grid-cols-[minmax(220px,360px)_1fr] md:items-end">
      <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
        Search
        <input
          class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="egress-profile-search"
          placeholder="Name, state, proxy, TLS"
          bind:value={search}
        />
      </label>
      <p class="m-0 text-sm text-admin-ink/62" data-testid="egress-profile-count">
        {rows.length} of {profiles.length} visible profiles
      </p>
    </div>
  </section>

  <div class="grid min-w-0 gap-3 xl:grid-cols-[minmax(260px,390px)_minmax(0,1fr)]">
    <section class="grid min-w-0 content-start gap-2 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Egress profiles">
      {#if rows.length === 0}
        <p class="m-0 text-sm text-admin-ink/62">No egress profiles match the current filter.</p>
      {:else}
        {#each rows as row (row.id)}
          <button
            class={`grid min-w-0 gap-2 rounded-xl border p-3 text-left transition ${rowClass(row.state, row.id === selectedProfileId)}`}
            type="button"
            data-testid="egress-profile-row"
            data-profile-id={row.id}
            aria-pressed={row.id === selectedProfileId}
            onclick={() => selectProfile(row.id)}
          >
            <span class="flex min-w-0 items-center justify-between gap-2">
              <span class="truncate text-sm font-bold">{row.name}</span>
              <span class={`shrink-0 rounded-lg border px-2 py-0.5 text-[11px] font-bold ${
                row.state === 'ready'
                  ? 'border-admin-leaf/30 bg-admin-leaf/12 text-admin-leaf'
                  : 'border-admin-danger/32 bg-admin-danger/10 text-admin-danger'
              }`}>
                {row.state}
              </span>
            </span>
            <span class="text-xs font-bold text-admin-ink/64" data-testid="egress-profile-row-health">
              {row.health} | {row.proofLevel.replaceAll('_', ' ')}
            </span>
            <span class="flex min-w-0 flex-wrap gap-1">
              {#each row.badges as badge}
                <span class="rounded-lg bg-admin-night/58 px-2 py-0.5 text-[11px] font-bold text-[#c1d0e8]">{badge}</span>
              {/each}
            </span>
            {#if row.description}
              <span class="line-clamp-2 text-xs leading-normal text-admin-ink/62">{row.description}</span>
            {/if}
          </button>
        {/each}
      {/if}
    </section>

    <section class="grid min-w-0 gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Egress profile details and editor">
      <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
        <div class="min-w-0">
          <p class="admin-eyebrow mb-1">Profile detail</p>
          <h3 class="m-0 text-base font-bold text-admin-ink" data-testid="egress-profile-selected-name">
            {selectedProfile?.name ?? 'No profile selected'}
          </h3>
        </div>
        <div class="flex min-w-0 flex-wrap gap-2">
          <button class="admin-button-primary inline-flex items-center gap-2" type="button" data-testid="egress-profile-edit" disabled={!selectedProfile || disabled} onclick={editSelected}>
            <Network size={15} aria-hidden="true" />
            Edit
          </button>
          <button class="admin-button-primary inline-flex items-center gap-2" type="button" data-testid="egress-profile-clone" disabled={!selectedProfile || disabled} onclick={cloneSelected}>
            <Copy size={15} aria-hidden="true" />
            Clone
          </button>
          <button
            class="admin-button-primary inline-flex items-center gap-2"
            type="button"
            disabled={!selectedProfile || selectedProfile.state === 'disabled' || disabled || !onUpdateProfile}
            data-testid="egress-profile-disable"
            onclick={() => void disableSelected()}
          >
            <ShieldOff size={15} aria-hidden="true" />
            Disable
          </button>
        </div>
      </div>

      {#if selectedProfile}
        <div class="grid min-w-0 gap-2 rounded-xl border border-admin-ink/10 bg-admin-field/62 p-3">
          <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
            <span class="rounded-lg bg-admin-night/58 px-2 py-1 text-xs font-bold text-[#c1d0e8]">State: {selectedProfile.state}</span>
            <span class="rounded-lg bg-admin-night/58 px-2 py-1 text-xs font-bold text-[#c1d0e8]" data-testid="egress-profile-health">Health: {selectedProfile.diagnostics.health}</span>
            <span class="rounded-lg bg-admin-night/58 px-2 py-1 text-xs font-bold text-[#c1d0e8]">Proxy: {selectedProfile.effective.proxy_configured ? 'configured' : 'none'}</span>
            <span class="rounded-lg bg-admin-night/58 px-2 py-1 text-xs font-bold text-[#c1d0e8]">TLS: {selectedProfile.effective.tls_interception_enabled ? 'inspect' : 'metadata'}</span>
            <span class="rounded-lg bg-admin-night/58 px-2 py-1 text-xs font-bold text-[#c1d0e8]">Bypass: {selectedProfile.effective.bypass_rule_count}</span>
          </div>
          <AdminMessage
            variant={selectedProfile.diagnostics.health === 'ready' ? 'info' : 'warning'}
            title={`Diagnostics: ${selectedProfile.diagnostics.proof_level.replaceAll('_', ' ')}`}
            message={selectedProfile.diagnostics.warnings.length > 0
              ? selectedProfile.diagnostics.warnings.join(' ')
              : selectedProfile.diagnostics.proof.active_probe_collected
                ? 'Active egress probe evidence is available for this profile.'
                : 'No active probe has been collected yet; proof is based on sanitized configuration metadata.'}
            compact={true}
          />
          <div class="grid min-w-0 gap-2 text-xs font-bold text-admin-ink/68 sm:grid-cols-2 lg:grid-cols-3" data-testid="egress-profile-diagnostics-proof">
            <span>Profile resolved: {selectedProfile.diagnostics.proof.profile_resolved ? 'yes' : 'no'}</span>
            <span>Runtime launch: {selectedProfile.diagnostics.proof.runtime_launch_observed ? 'observed' : 'not observed'}</span>
            <span>Active probe: {selectedProfile.diagnostics.proof.active_probe_collected ? 'collected' : 'not collected'}</span>
            <span>TLS expected: {selectedProfile.diagnostics.proof.tls_interception_expected ? 'yes' : 'no'}</span>
            <span>Custom CA launch: {selectedProfile.diagnostics.proof.custom_ca_launch_config_expected ? 'expected' : 'not expected'}</span>
            <span>Log sink: {selectedProfile.diagnostics.proof.sensitive_log_sink_declared ? 'declared' : 'not declared'}</span>
          </div>
        </div>
      {/if}

      <form class="grid min-w-0 gap-3 rounded-xl border border-admin-ink/10 bg-admin-field/62 p-3" onsubmit={(event) => { event.preventDefault(); void saveProfile(); }}>
        <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
          <h4 class="m-0 text-sm font-bold text-admin-ink">{editing ? 'Edit egress profile' : 'Create egress profile'}</h4>
          <button class="admin-button-primary inline-flex items-center gap-2" type="button" data-testid="egress-profile-new" disabled={disabled} onclick={resetForm}>
            <ShieldCheck size={15} aria-hidden="true" />
            New profile
          </button>
        </div>

        <div class="grid min-w-0 gap-3 md:grid-cols-2">
          <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
            Name
            <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-panel/78 px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-name" bind:value={form.name} disabled={disabled} />
          </label>
          <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
            State
            <select class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-panel/78 px-3 text-admin-ink outline-none focus:border-admin-leaf/45" bind:value={form.state} disabled={disabled}>
              <option value="ready">ready</option>
              <option value="disabled">disabled</option>
            </select>
          </label>
        </div>

        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Description
          <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-panel/78 px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-description" bind:value={form.description} disabled={disabled} />
        </label>

        <div class="grid min-w-0 gap-3 md:grid-cols-2">
          <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
            Labels
            <textarea class="min-h-24 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-panel/78 p-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-labels" placeholder="region=eu" bind:value={form.labels} disabled={disabled}></textarea>
          </label>
          <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
            Bypass rules
            <textarea class="min-h-24 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-panel/78 p-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-bypass-rules" placeholder="localhost&#10;*.local" bind:value={form.bypassRules} disabled={disabled}></textarea>
          </label>
        </div>

        <div class="grid min-w-0 gap-3 md:grid-cols-2">
          <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
            Proxy URL
            <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-panel/78 px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-proxy-url" placeholder="http://bpane-egress-observer:3128" bind:value={form.proxyUrl} disabled={disabled} />
          </label>
          <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
            Observation mode
            <select class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-panel/78 px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-observation-mode" bind:value={form.observationMode} disabled={disabled}>
              <option value="metadata_only">metadata_only</option>
              <option value="tls_intercept">tls_intercept</option>
            </select>
          </label>
        </div>

        <div class="grid min-w-0 gap-3 md:grid-cols-2">
          <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
            Custom CA ref
            <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-panel/78 px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-custom-ca-ref" placeholder="file:///workspace/dev/egress-ca.pem" bind:value={form.customCaRef} disabled={disabled} />
          </label>
          <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
            Custom CA name
            <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-panel/78 px-3 text-admin-ink outline-none focus:border-admin-leaf/45" bind:value={form.customCaName} disabled={disabled} />
          </label>
        </div>

        <div class="grid min-w-0 gap-3 md:grid-cols-2">
          <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
            Log-sink ref
            <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-panel/78 px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-log-sink-ref" placeholder="siem://browserpane/local-egress" bind:value={form.sensitiveLogSinkRef} disabled={disabled} />
          </label>
          <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
            Log-sink name
            <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-panel/78 px-3 text-admin-ink outline-none focus:border-admin-leaf/45" bind:value={form.sensitiveLogSinkName} disabled={disabled} />
          </label>
        </div>

        <button
          class="admin-button-primary inline-flex w-fit items-center gap-2"
          type="submit"
          data-testid="egress-profile-save"
          disabled={disabled || !canSave}
        >
          {#if mutating}
            <RefreshCw class="animate-spin" size={15} aria-hidden="true" />
            Saving
          {:else}
            <ShieldCheck size={15} aria-hidden="true" />
            {editing ? 'Save profile' : 'Create profile'}
          {/if}
        </button>
      </form>
    </section>
  </div>
</div>
