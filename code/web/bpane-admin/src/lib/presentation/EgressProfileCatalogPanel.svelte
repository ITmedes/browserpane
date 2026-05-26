<script lang="ts">
  import { Copy, Network, RefreshCw, ShieldCheck, ShieldOff } from 'lucide-svelte';
  import type {
    CreateEgressProfileCommand,
    EgressDiagnosticsResource,
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
    readonly onRunProfileReachabilityProbe?: (profileId: string) => Promise<EgressDiagnosticsResource | void> | EgressDiagnosticsResource | void;
  };

  type EgressProfileEditorMode = 'create' | 'edit' | 'clone';

  let {
    profiles,
    loading = false,
    error = null,
    onRefresh,
    onCreateProfile,
    onUpdateProfile,
    onRunProfileReachabilityProbe,
  }: EgressProfileCatalogPanelProps = $props();

  let search = $state('');
  let selectedProfileId = $state<string | null>(null);
  let editingProfileId = $state<string | null>(null);
  let editorOpen = $state(false);
  let editorMode = $state<EgressProfileEditorMode>('create');
  let form = $state(emptyEgressProfileForm());
  let mutating = $state(false);
  let feedback = $state<AdminMessageFeedback | null>(null);
  const rows = $derived(egressProfileRows(profiles, search));
  const selectedProfile = $derived(profiles.find((profile) => profile.id === selectedProfileId) ?? null);
  const selectedRow = $derived(rows.find((row) => row.id === selectedProfileId) ?? null);
  const editing = $derived(Boolean(editingProfileId));
  const canSave = $derived(editorOpen && (Boolean(onCreateProfile && !editing) || Boolean(onUpdateProfile && editing)));
  const disabled = $derived(loading || mutating);
  const editorTitle = $derived(
    editorMode === 'edit'
      ? 'Edit egress profile'
      : editorMode === 'clone'
        ? 'Clone egress profile'
        : 'Create egress profile',
  );

  $effect(() => {
    if (!rows.length) {
      selectedProfileId = null;
    } else if (!rows.some((row) => row.id === selectedProfileId)) {
      selectedProfileId = rows[0]?.id ?? null;
    }
  });

  function selectProfile(profileId: string): void {
    if (profileId !== selectedProfileId && editorOpen && editingProfileId && editingProfileId !== profileId) {
      closeEditor();
    }
    selectedProfileId = profileId;
    feedback = null;
  }

  function resetForm(): void {
    editingProfileId = null;
    editorMode = 'create';
    editorOpen = true;
    form = emptyEgressProfileForm();
    feedback = null;
  }

  function closeEditor(): void {
    editorOpen = false;
    editingProfileId = null;
    form = emptyEgressProfileForm();
  }

  function editSelected(): void {
    const profile = selectedProfile;
    if (!profile) {
      return;
    }
    editingProfileId = profile.id;
    editorMode = 'edit';
    editorOpen = true;
    form = formFromEgressProfile(profile);
    feedback = null;
  }

  function cloneSelected(): void {
    const profile = selectedProfile;
    if (!profile) {
      return;
    }
    editingProfileId = null;
    editorMode = 'clone';
    editorOpen = true;
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
      editorOpen = false;
      form = emptyEgressProfileForm();
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

  async function runReachabilityProbe(): Promise<void> {
    const profile = selectedProfile;
    if (!profile || !onRunProfileReachabilityProbe) {
      return;
    }
    mutating = true;
    feedback = null;
    try {
      const diagnostics = await onRunProfileReachabilityProbe(profile.id);
      const healthy = diagnostics?.proof.profile_reachability_healthy ?? false;
      feedback = {
        variant: healthy ? 'success' : 'warning',
        title: healthy ? 'Profile reachability verified' : 'Profile reachability failed',
        message: healthy
          ? `${profile.name} can reach its configured egress endpoint.`
          : diagnostics?.proof.profile_reachability_failure ?? 'The egress endpoint could not be reached from the gateway.',
      };
      onRefresh();
    } catch (probeError) {
      feedback = {
        variant: 'error',
        title: 'Profile reachability probe failed',
        message: probeError instanceof Error ? probeError.message : 'Could not run profile reachability diagnostics.',
      };
    } finally {
      mutating = false;
    }
  }

  function rowClass(state: string, selected: boolean): string {
    if (selected) {
      return 'border-admin-leaf/42 bg-admin-field/84';
    }
    return state === 'disabled'
      ? 'border-admin-danger/24 bg-admin-danger/8'
      : 'border-admin-ink/10 bg-admin-panel/68';
  }

  function rowAccentClass(state: string, selected: boolean): string {
    if (selected) {
      return 'bg-admin-leaf';
    }
    return state === 'disabled' ? 'bg-admin-danger/62' : 'bg-admin-ink/12';
  }
</script>

<div class="grid min-w-0 gap-3" data-testid="egress-profile-catalog">
  <section class="grid gap-3 rounded-[16px] border border-admin-leaf/25 bg-admin-leaf/10 p-3" aria-label="Selected egress profile">
    <div class="flex min-w-0 flex-wrap items-start justify-between gap-2">
      <div class="min-w-0">
        <p class="admin-eyebrow mb-1">Selected egress profile</p>
        <h3 class="m-0 truncate text-base font-bold text-admin-ink" data-testid="egress-profile-selected-name" title={selectedProfile?.name ?? ''}>
          {selectedProfile?.name ?? 'No profile selected'}
        </h3>
      </div>
      <span class={`rounded-full border px-3 py-1 text-xs font-extrabold ${
        !selectedProfile
          ? 'border-[#90a6cc]/28 bg-admin-field/72 text-admin-ink/68'
          : selectedProfile.state === 'ready'
          ? 'border-admin-leaf/30 bg-admin-leaf/12 text-admin-leaf'
          : 'border-admin-danger/32 bg-admin-danger/10 text-admin-danger'
      }`}>
        {selectedProfile?.state ?? 'select'}
      </span>
    </div>

    {#if selectedProfile}
      <div class="grid min-w-0 grid-cols-2 gap-2 text-xs text-admin-ink/70 sm:grid-cols-3 xl:grid-cols-6">
        {@render ProfileFact('Mode', selectedRow?.kind ?? 'direct')}
        {@render ProfileFact('Health', selectedProfile.diagnostics.health, 'egress-profile-health')}
        {@render ProfileFact('Proxy', selectedProfile.effective.proxy_configured ? 'configured' : 'none')}
        {@render ProfileFact('Proxy auth', selectedProfile.effective.proxy_auth_configured ? 'binding' : 'none')}
        {@render ProfileFact('TLS', selectedProfile.effective.tls_interception_enabled ? 'intercept' : 'metadata')}
        {@render ProfileFact('Bypass', String(selectedProfile.effective.bypass_rule_count))}
      </div>
      <p class="m-0 line-clamp-2 text-sm leading-normal text-admin-ink/64">
        {selectedProfile.description ?? 'No description configured for this outbound path.'}
      </p>
    {:else}
      <p class="m-0 text-sm leading-normal text-admin-ink/68">
        Select an approved outbound path before editing, cloning, disabling, or probing an egress profile.
      </p>
    {/if}

    <div class="flex flex-wrap gap-2">
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
      <button
        class="admin-button-primary inline-flex items-center gap-2"
        type="button"
        disabled={!selectedProfile || disabled || !onRunProfileReachabilityProbe}
        data-testid="egress-profile-reachability-probe"
        onclick={() => void runReachabilityProbe()}
      >
        <RefreshCw class={mutating ? 'animate-spin' : ''} size={15} aria-hidden="true" />
        Probe
      </button>
    </div>
  </section>

  {#if error}
    <AdminMessage variant="warning" title="Egress catalog unavailable" message={error} compact={true} />
  {/if}
  {#if feedback}
    <AdminMessage variant={feedback.variant} title={feedback.title} message={feedback.message} compact={true} onDismiss={() => { feedback = null; }} />
  {/if}

  <section class="grid min-w-0 gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Egress profile configurator">
    <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
      <div class="min-w-0">
        <p class="admin-eyebrow mb-1">Profile configurator</p>
        <h4 class="m-0 text-sm font-bold text-admin-ink">{editorOpen ? editorTitle : 'Create or update an approved egress path'}</h4>
      </div>
      <div class="flex flex-wrap gap-2">
        <button class="admin-button-primary inline-flex items-center gap-2" type="button" data-testid="egress-profile-new" disabled={disabled} onclick={resetForm}>
          <ShieldCheck size={15} aria-hidden="true" />
          New profile
        </button>
        {#if editorOpen}
          <button class="admin-button-ghost" type="button" disabled={disabled} onclick={closeEditor}>
            Cancel
          </button>
        {/if}
      </div>
    </div>

    {#if !editorOpen}
      <AdminMessage
        variant="info"
        message="Use New profile to define a new outbound path, or select a profile above and use Edit or Clone."
        compact={true}
      />
    {:else}
    <form class="grid min-w-0 gap-3" aria-label="Egress profile editor" onsubmit={(event) => { event.preventDefault(); void saveProfile(); }}>
      <div class="grid min-w-0 gap-3 md:grid-cols-2">
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Name
          <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-name" bind:value={form.name} disabled={disabled} />
        </label>
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          State
          <select class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" bind:value={form.state} disabled={disabled}>
            <option value="ready">ready</option>
            <option value="disabled">disabled</option>
          </select>
        </label>
      </div>

      <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
        Description
        <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-description" bind:value={form.description} disabled={disabled} />
      </label>

      <div class="grid min-w-0 gap-3 md:grid-cols-2">
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Labels
          <textarea class="min-h-24 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field p-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-labels" placeholder="region=eu" bind:value={form.labels} disabled={disabled}></textarea>
        </label>
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Bypass rules
          <textarea class="min-h-24 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field p-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-bypass-rules" placeholder="localhost&#10;*.local" bind:value={form.bypassRules} disabled={disabled}></textarea>
        </label>
      </div>

      <div class="grid min-w-0 gap-3 md:grid-cols-2">
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Proxy URL
          <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-proxy-url" placeholder="http://bpane-egress-observer:3128" bind:value={form.proxyUrl} disabled={disabled} />
        </label>
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Proxy auth binding ID
          <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-proxy-credential-binding-id" placeholder="credential binding UUID" bind:value={form.proxyCredentialBindingId} disabled={disabled} />
        </label>
      </div>

      <div class="grid min-w-0 gap-3 md:grid-cols-2">
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Observation mode
          <select class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-observation-mode" bind:value={form.observationMode} disabled={disabled}>
            <option value="metadata_only">metadata_only</option>
            <option value="tls_intercept">tls_intercept</option>
          </select>
        </label>
      </div>

      <div class="grid min-w-0 gap-3 md:grid-cols-2">
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Custom CA ref
          <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-custom-ca-ref" placeholder="file:///workspace/dev/egress-ca.pem" bind:value={form.customCaRef} disabled={disabled} />
        </label>
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Custom CA name
          <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" bind:value={form.customCaName} disabled={disabled} />
        </label>
      </div>

      <div class="grid min-w-0 gap-3 md:grid-cols-2">
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Log-sink ref
          <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="egress-profile-log-sink-ref" placeholder="siem://browserpane/local-egress" bind:value={form.sensitiveLogSinkRef} disabled={disabled} />
        </label>
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Log-sink name
          <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" bind:value={form.sensitiveLogSinkName} disabled={disabled} />
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
    {/if}
  </section>

  <section class="grid min-w-0 gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Egress profile switcher">
    <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
      <div class="min-w-0">
        <p class="admin-eyebrow mb-1">Profile switcher</p>
        <p class="m-0 text-sm font-bold text-admin-ink/72" data-testid="egress-profile-count">
          {rows.length} of {profiles.length} visible profiles
        </p>
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

    <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
      Search
      <input
        class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
        data-testid="egress-profile-search"
        placeholder="Name, state, proxy, TLS"
        bind:value={search}
      />
    </label>

    {#if rows.length === 0}
      <AdminMessage variant="empty" message="No egress profiles match the current filter." compact={true} />
    {:else}
      <div class="grid max-h-[min(360px,42vh)] min-w-0 gap-1 overflow-y-auto pr-1" aria-label="Visible egress profiles">
        {#each rows as row (row.id)}
          <button
            class={`grid w-full min-w-0 cursor-pointer grid-cols-[4px_minmax(0,1fr)_auto] items-center gap-3 rounded-xl border p-2 text-left text-admin-ink/78 hover:border-admin-leaf/42 hover:bg-admin-field/84 ${rowClass(row.state, row.id === selectedProfileId)}`}
            type="button"
            data-testid="egress-profile-row"
            data-profile-id={row.id}
            aria-pressed={row.id === selectedProfileId}
            onclick={() => selectProfile(row.id)}
          >
            <span class={`h-full min-h-12 rounded-full ${rowAccentClass(row.state, row.id === selectedProfileId)}`}></span>
            <span class="grid min-w-0 gap-1">
              <span class="flex min-w-0 items-center gap-2">
                <strong class="min-w-0 truncate text-sm text-admin-ink" title={row.name}>{row.name}</strong>
                {#if row.id === selectedProfileId}
                  <span class="rounded-full bg-admin-leaf/14 px-2 py-0.5 text-[0.68rem] font-extrabold text-admin-leaf">selected</span>
                {/if}
              </span>
              <span class="min-w-0 truncate text-xs text-admin-ink/52">
                <span data-testid="egress-profile-row-health">{row.kind}</span> | {row.description || 'no description'} | updated {row.updatedAt}
              </span>
            </span>
            <span class="grid justify-items-end gap-1 text-xs text-[#c1d0e8]">
              <span class="rounded-lg bg-admin-field/72 px-2 py-1">{row.state}</span>
              <span class="rounded-lg bg-admin-field/72 px-2 py-1">{row.kind}</span>
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </section>
</div>

{#snippet ProfileFact(label: string, value: string, testId?: string)}
  <span class="min-w-0 rounded-xl bg-admin-field/72 p-2 font-bold uppercase">
    {label}
    <strong class="mt-1 block truncate font-mono text-admin-ink normal-case" data-testid={testId}>{value}</strong>
  </span>
{/snippet}
