<script lang="ts">
  import { KeyRound, Pencil, Plus, RefreshCw, ShieldCheck, ShieldOff } from 'lucide-svelte';
  import { tick } from 'svelte';
  import type {
    CreateIdentityMappingCommand,
    CreateServicePrincipalCommand,
    IdentityAccessReviewResponse,
    IdentityMappingKind,
    IdentityMappingResource,
    IdentityMappingState,
    ServicePrincipalResource,
    ServicePrincipalState,
  } from '../api/control-types';
  import AdminMessage from './AdminMessage.svelte';
  import { IdentityAccessReviewViewModelBuilder } from './identity-access-review-view-model';
  import {
    buildIdentityMappingCommand,
    commandFromIdentityMapping,
    emptyIdentityMappingForm,
    formFromIdentityMapping,
    formWithServicePrincipal,
    identityMappingRows,
  } from './identity-mapping-catalog';
  import {
    buildServicePrincipalCommand,
    commandFromServicePrincipal,
    emptyServicePrincipalForm,
    formFromServicePrincipal,
    servicePrincipalRows as buildServicePrincipalRows,
  } from './service-principal-catalog';

  type IdentityAccessReviewPanelProps = {
    readonly review: IdentityAccessReviewResponse | null;
    readonly loading?: boolean;
    readonly error?: string | null;
    readonly onRefresh: () => void;
    readonly onCreateMapping?: (command: CreateIdentityMappingCommand) => Promise<IdentityMappingResource | void> | IdentityMappingResource | void;
    readonly onUpdateMapping?: (mappingId: string, command: CreateIdentityMappingCommand) => Promise<IdentityMappingResource | void> | IdentityMappingResource | void;
    readonly onCreateServicePrincipal?: (command: CreateServicePrincipalCommand) => Promise<ServicePrincipalResource | void> | ServicePrincipalResource | void;
    readonly onUpdateServicePrincipal?: (servicePrincipalId: string, command: CreateServicePrincipalCommand) => Promise<ServicePrincipalResource | void> | ServicePrincipalResource | void;
  };

  let {
    review,
    loading = false,
    error = null,
    onRefresh,
    onCreateMapping,
    onUpdateMapping,
    onCreateServicePrincipal,
    onUpdateServicePrincipal,
  }: IdentityAccessReviewPanelProps = $props();

  let servicePrincipalSearch = $state('');
  let selectedServicePrincipalId = $state<string | null>(null);
  let servicePrincipalEditorSection = $state<HTMLElement | null>(null);
  let servicePrincipalNameInput = $state<HTMLInputElement | null>(null);
  let servicePrincipalEditorOpen = $state(false);
  let editingServicePrincipalId = $state<string | null>(null);
  let servicePrincipalMutating = $state(false);
  let servicePrincipalFeedback = $state<{ readonly variant: 'success' | 'error' | 'warning' | 'info'; readonly title?: string; readonly message: string } | null>(null);
  let servicePrincipalForm = $state(emptyServicePrincipalForm(null));
  let mappingSearch = $state('');
  let selectedMappingId = $state<string | null>(null);
  let editorSection = $state<HTMLElement | null>(null);
  let mappingNameInput = $state<HTMLInputElement | null>(null);
  let mappingEditorOpen = $state(false);
  let editingMappingId = $state<string | null>(null);
  let mappingMutating = $state(false);
  let mappingFeedback = $state<{ readonly variant: 'success' | 'error' | 'warning' | 'info'; readonly title?: string; readonly message: string } | null>(null);
  let mappingForm = $state(emptyIdentityMappingForm(null, []));
  const viewModel = $derived(review ? IdentityAccessReviewViewModelBuilder.build(review) : null);
  const mappings = $derived(review?.identity_mappings ?? []);
  const mappingRows = $derived(identityMappingRows(mappings, mappingSearch, projects));
  const selectedMapping = $derived(mappings.find((mapping) => mapping.id === selectedMappingId) ?? null);
  const selectedMappingRow = $derived(mappingRows.find((row) => row.id === selectedMappingId) ?? null);
  const projects = $derived(review?.projects ?? []);
  const activeProjects = $derived(projects.filter((project) => project.state === 'active'));
  const servicePrincipals = $derived(review?.service_principals ?? []);
  const servicePrincipalRows = $derived(buildServicePrincipalRows(servicePrincipals, servicePrincipalSearch, projects));
  const selectedServicePrincipal = $derived(servicePrincipals.find((servicePrincipal) => servicePrincipal.id === selectedServicePrincipalId) ?? null);
  const selectedServicePrincipalRow = $derived(servicePrincipalRows.find((row) => row.id === selectedServicePrincipalId) ?? null);
  const disabled = $derived(loading || mappingMutating || servicePrincipalMutating);
  const mappingEditorTitle = $derived(editingMappingId ? 'Edit identity mapping' : 'Create identity mapping');
  const mappingSaveLabel = $derived(editingMappingId ? 'Save mapping' : 'Create mapping');
  const servicePrincipalEditorTitle = $derived(editingServicePrincipalId ? 'Edit service principal' : 'Create service principal');
  const servicePrincipalSaveLabel = $derived(editingServicePrincipalId ? 'Save service principal' : 'Create service principal');

  $effect(() => {
    if (!mappingRows.length) {
      selectedMappingId = null;
    } else if (!mappingRows.some((row) => row.id === selectedMappingId)) {
      selectedMappingId = mappingRows[0]?.id ?? null;
    }
  });

  $effect(() => {
    if (!servicePrincipalRows.length) {
      selectedServicePrincipalId = null;
    } else if (!servicePrincipalRows.some((row) => row.id === selectedServicePrincipalId)) {
      selectedServicePrincipalId = servicePrincipalRows[0]?.id ?? null;
    }
  });

  function selectServicePrincipal(servicePrincipalId: string): void {
    selectedServicePrincipalId = servicePrincipalId;
    if (editingServicePrincipalId && editingServicePrincipalId !== servicePrincipalId) {
      closeServicePrincipalEditor();
    }
    servicePrincipalFeedback = null;
  }

  function openCreateServicePrincipal(): void {
    editingServicePrincipalId = null;
    servicePrincipalEditorOpen = true;
    servicePrincipalForm = emptyServicePrincipalForm(review?.principal ?? null);
    servicePrincipalFeedback = null;
    void revealServicePrincipalEditor();
  }

  function editSelectedServicePrincipal(): void {
    const servicePrincipal = selectedServicePrincipal;
    if (!servicePrincipal) {
      return;
    }
    editingServicePrincipalId = servicePrincipal.id;
    servicePrincipalEditorOpen = true;
    servicePrincipalForm = formFromServicePrincipal(servicePrincipal);
    servicePrincipalFeedback = null;
    void revealServicePrincipalEditor();
  }

  function closeServicePrincipalEditor(): void {
    servicePrincipalEditorOpen = false;
    editingServicePrincipalId = null;
    servicePrincipalForm = emptyServicePrincipalForm(review?.principal ?? null);
  }

  function setAllowedProjects(event: Event): void {
    const target = event.currentTarget instanceof HTMLSelectElement ? event.currentTarget : null;
    servicePrincipalForm = {
      ...servicePrincipalForm,
      allowedProjectIds: target ? Array.from(target.selectedOptions).map((option) => option.value) : [],
    };
  }

  async function revealServicePrincipalEditor(): Promise<void> {
    await tick();
    servicePrincipalEditorSection?.scrollIntoView({ block: 'start', behavior: 'auto' });
    servicePrincipalNameInput?.focus({ preventScroll: true });
    servicePrincipalNameInput?.select();
  }

  async function saveServicePrincipal(): Promise<void> {
    const result = buildServicePrincipalCommand(servicePrincipalForm);
    if (!result.ok) {
      servicePrincipalFeedback = { variant: 'error', title: 'Service principal validation failed', message: result.error };
      return;
    }
    if (editingServicePrincipalId && !onUpdateServicePrincipal) {
      servicePrincipalFeedback = { variant: 'error', title: 'Service principal update unavailable', message: 'This admin view cannot update service principals.' };
      return;
    }
    if (!editingServicePrincipalId && !onCreateServicePrincipal) {
      servicePrincipalFeedback = { variant: 'error', title: 'Service principal create unavailable', message: 'This admin view cannot create service principals.' };
      return;
    }

    servicePrincipalMutating = true;
    servicePrincipalFeedback = null;
    try {
      const saved = editingServicePrincipalId
        ? await onUpdateServicePrincipal?.(editingServicePrincipalId, result.command)
        : await onCreateServicePrincipal?.(result.command);
      if (saved?.id) {
        selectedServicePrincipalId = saved.id;
      }
      servicePrincipalFeedback = {
        variant: 'success',
        title: editingServicePrincipalId ? 'Service principal updated' : 'Service principal created',
        message: saved?.name ? `${saved.name} is available in access review.` : 'Service principal saved.',
      };
      servicePrincipalEditorOpen = false;
      editingServicePrincipalId = null;
    } catch (saveError) {
      servicePrincipalFeedback = {
        variant: 'error',
        title: 'Service principal save failed',
        message: saveError instanceof Error ? saveError.message : 'Could not save service principal.',
      };
    } finally {
      servicePrincipalMutating = false;
    }
  }

  async function setSelectedServicePrincipalState(state: ServicePrincipalState): Promise<void> {
    const servicePrincipal = selectedServicePrincipal;
    if (!servicePrincipal || !onUpdateServicePrincipal || servicePrincipal.state === state) {
      return;
    }
    servicePrincipalMutating = true;
    servicePrincipalFeedback = null;
    try {
      const saved = await onUpdateServicePrincipal(servicePrincipal.id, commandFromServicePrincipal(servicePrincipal, state));
      if (saved?.id) {
        selectedServicePrincipalId = saved.id;
      }
      servicePrincipalFeedback = {
        variant: 'success',
        title: state === 'active' ? 'Service principal enabled' : 'Service principal disabled',
        message: `${servicePrincipal.name} is now ${state}.`,
      };
    } catch (stateError) {
      servicePrincipalFeedback = {
        variant: 'error',
        title: 'Service principal state change failed',
        message: stateError instanceof Error ? stateError.message : 'Could not update service principal state.',
      };
    } finally {
      servicePrincipalMutating = false;
    }
  }

  function selectMapping(mappingId: string): void {
    selectedMappingId = mappingId;
    if (editingMappingId && editingMappingId !== mappingId) {
      closeMappingEditor();
    }
    mappingFeedback = null;
  }

  function openCreateMapping(): void {
    editingMappingId = null;
    mappingEditorOpen = true;
    mappingForm = emptyIdentityMappingForm(review?.principal ?? null, activeProjects.length > 0 ? activeProjects : projects);
    mappingFeedback = null;
    void revealMappingEditor();
  }

  function editSelectedMapping(): void {
    const mapping = selectedMapping;
    if (!mapping) {
      return;
    }
    editingMappingId = mapping.id;
    mappingEditorOpen = true;
    mappingForm = formFromIdentityMapping(mapping);
    mappingFeedback = null;
    void revealMappingEditor();
  }

  function closeMappingEditor(): void {
    mappingEditorOpen = false;
    editingMappingId = null;
    mappingForm = emptyIdentityMappingForm(review?.principal ?? null, activeProjects.length > 0 ? activeProjects : projects);
  }

  function setMappingKind(event: Event): void {
    const target = event.currentTarget instanceof HTMLSelectElement ? event.currentTarget : null;
    const kind = (target?.value ?? 'user') as IdentityMappingKind;
    if (kind === 'service_principal') {
      const selectedServicePrincipal = servicePrincipals.find((principal) => principal.id === mappingForm.servicePrincipalId)
        ?? servicePrincipals[0]
        ?? null;
      mappingForm = formWithServicePrincipal({ ...mappingForm, kind }, selectedServicePrincipal);
      return;
    }
    mappingForm = {
      ...mappingForm,
      kind,
      claimName: kind === 'claim' ? mappingForm.claimName : '',
      servicePrincipalId: '',
    };
  }

  function setServicePrincipal(event: Event): void {
    const target = event.currentTarget instanceof HTMLSelectElement ? event.currentTarget : null;
    const servicePrincipalId = target?.value ?? '';
    const selectedServicePrincipal = servicePrincipals.find((principal) => principal.id === servicePrincipalId) ?? null;
    mappingForm = formWithServicePrincipal(mappingForm, selectedServicePrincipal);
  }

  async function revealMappingEditor(): Promise<void> {
    await tick();
    editorSection?.scrollIntoView({ block: 'start', behavior: 'auto' });
    mappingNameInput?.focus({ preventScroll: true });
    mappingNameInput?.select();
  }

  async function saveMapping(): Promise<void> {
    const result = buildIdentityMappingCommand(mappingForm);
    if (!result.ok) {
      mappingFeedback = { variant: 'error', title: 'Mapping validation failed', message: result.error };
      return;
    }
    if (editingMappingId && !onUpdateMapping) {
      mappingFeedback = { variant: 'error', title: 'Mapping update unavailable', message: 'This admin view cannot update identity mappings.' };
      return;
    }
    if (!editingMappingId && !onCreateMapping) {
      mappingFeedback = { variant: 'error', title: 'Mapping create unavailable', message: 'This admin view cannot create identity mappings.' };
      return;
    }

    mappingMutating = true;
    mappingFeedback = null;
    try {
      const saved = editingMappingId
        ? await onUpdateMapping?.(editingMappingId, result.command)
        : await onCreateMapping?.(result.command);
      if (saved?.id) {
        selectedMappingId = saved.id;
      }
      mappingFeedback = {
        variant: 'success',
        title: editingMappingId ? 'Mapping updated' : 'Mapping created',
        message: saved?.name ? `${saved.name} is available in access review.` : 'Identity mapping saved.',
      };
      mappingEditorOpen = false;
      editingMappingId = null;
    } catch (saveError) {
      mappingFeedback = {
        variant: 'error',
        title: 'Mapping save failed',
        message: saveError instanceof Error ? saveError.message : 'Could not save identity mapping.',
      };
    } finally {
      mappingMutating = false;
    }
  }

  async function setSelectedMappingState(state: IdentityMappingState): Promise<void> {
    const mapping = selectedMapping;
    if (!mapping || !onUpdateMapping || mapping.state === state) {
      return;
    }
    mappingMutating = true;
    mappingFeedback = null;
    try {
      const saved = await onUpdateMapping(mapping.id, commandFromIdentityMapping(mapping, state));
      if (saved?.id) {
        selectedMappingId = saved.id;
      }
      mappingFeedback = {
        variant: 'success',
        title: state === 'active' ? 'Mapping enabled' : 'Mapping disabled',
        message: `${mapping.name} is now ${state}.`,
      };
    } catch (stateError) {
      mappingFeedback = {
        variant: 'error',
        title: 'Mapping state change failed',
        message: stateError instanceof Error ? stateError.message : 'Could not update identity mapping state.',
      };
    } finally {
      mappingMutating = false;
    }
  }

  function mappingRowClass(state: string, selected: boolean): string {
    if (selected) {
      return 'border-admin-leaf/42 bg-admin-field/84';
    }
    return state === 'disabled'
      ? 'border-admin-danger/24 bg-admin-danger/8'
      : 'border-admin-ink/10 bg-admin-panel/68';
  }

  function mappingAccentClass(state: string, selected: boolean): string {
    if (selected) {
      return 'bg-admin-leaf';
    }
    return state === 'disabled' ? 'bg-admin-danger/62' : 'bg-admin-ink/12';
  }

  function servicePrincipalRowClass(state: string, selected: boolean): string {
    if (selected) {
      return 'border-admin-leaf/42 bg-admin-field/84';
    }
    return state === 'disabled'
      ? 'border-admin-danger/24 bg-admin-danger/8'
      : 'border-admin-ink/10 bg-admin-panel/68';
  }
</script>

<section class="grid min-w-0 gap-4" aria-label="Identity access review" data-testid="identity-access-review-panel">
  <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
    <p class="m-0 text-sm leading-normal text-admin-ink/68">
      Review the authenticated principal, owner-scoped resource counts, identity mappings, unmapped signals, and delegated automation access.
    </p>
    <button
      class="admin-button-ghost inline-flex items-center gap-2"
      type="button"
      data-testid="identity-refresh"
      disabled={loading}
      onclick={onRefresh}
    >
      <RefreshCw size={15} class={loading ? 'animate-spin' : ''} aria-hidden="true" />
      Refresh
    </button>
  </div>

  {#if error}
    <AdminMessage
      variant="warning"
      title="Identity review unavailable"
      message={error}
      testId="identity-access-review-error"
      compact={true}
    />
  {/if}

  {#if loading && !viewModel}
    <AdminMessage
      variant="loading"
      title="Loading identity review"
      message="Fetching the current principal and access summary."
      testId="identity-access-review-loading"
      compact={true}
    />
  {:else if !viewModel}
    <AdminMessage
      variant="empty"
      title="No identity review loaded"
      message="Refresh the panel to load the current access review."
      testId="identity-access-review-empty"
      compact={true}
    />
  {:else}
    <section class="rounded-[14px] border border-[#90a6cc]/18 bg-[#111e32]/72 p-3" aria-label="Current principal">
      <div class="grid min-w-0 gap-2">
        <p class="admin-eyebrow m-0">Current principal</p>
        <div class="grid min-w-0 gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-start">
          <div class="min-w-0">
            <strong class="block truncate text-base font-extrabold text-admin-ink" data-testid="identity-principal-title">
              {viewModel.principalTitle}
            </strong>
            <p class="m-0 min-w-0 text-xs font-semibold leading-normal text-[#c1d0e8] [overflow-wrap:anywhere]" data-testid="identity-principal-subtitle">
              {viewModel.principalSubtitle}
            </p>
          </div>
          <span class="w-fit rounded-xl border border-admin-leaf/24 bg-admin-leaf/10 px-3 py-1 text-xs font-bold text-admin-leaf" data-testid="identity-principal-type">
            {viewModel.principalTypeLabel}
          </span>
        </div>
        <p class="m-0 text-xs font-semibold text-admin-ink/58" data-testid="identity-review-generated-at">
          Generated {viewModel.generatedAtLabel}
        </p>
      </div>
    </section>

    <div class="grid grid-cols-2 gap-2 max-[760px]:grid-cols-1" data-testid="identity-resource-counts">
      {#each viewModel.metrics as metric (metric.key)}
        <span class="rounded-[12px] bg-admin-leaf/10 p-3 text-xs font-bold text-admin-ink/62 uppercase">
          {metric.label}
          <strong class="mt-1 block text-admin-ink normal-case" data-testid={metric.testId}>{metric.value}</strong>
        </span>
      {/each}
    </div>

    <section class="grid min-w-0 gap-2" aria-label="Project access" data-testid="identity-project-list">
      <div class="flex items-center justify-between gap-2">
        <p class="admin-eyebrow m-0">Projects</p>
        <span class="text-xs font-bold text-admin-ink/58">{viewModel.projects.length}</span>
      </div>
      {#if viewModel.projects.length === 0}
        <AdminMessage variant="empty" message="No projects are visible for this principal." compact={true} />
      {:else}
        <div class="grid gap-2">
          {#each viewModel.projects as project (project.id)}
            <article class="rounded-[12px] border border-[#90a6cc]/16 bg-admin-panel/62 p-3">
              <div class="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-start">
                <div class="min-w-0">
                  <strong class="block truncate text-sm font-extrabold text-admin-ink">{project.name}</strong>
                  <p class="m-0 truncate text-xs font-semibold text-admin-ink/54">{project.id}</p>
                </div>
                <span class="w-fit rounded-lg bg-admin-cream/72 px-2 py-1 text-xs font-bold text-admin-ink/68">{project.state}</span>
              </div>
              <div class="mt-3 grid grid-cols-4 gap-2 max-[900px]:grid-cols-2 max-[760px]:grid-cols-1">
                <span class="text-xs font-bold text-admin-ink/58">Sessions <strong class="block text-admin-ink">{project.activeSessions}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Queued <strong class="block text-admin-ink">{project.queuedSessions}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Created <strong class="block text-admin-ink">{project.sessionCreations}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Workflows <strong class="block text-admin-ink">{project.activeWorkflowRuns}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Runtime <strong class="block text-admin-ink">{project.runtimeUsage}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Egress <strong class="block text-admin-ink">{project.egressUsage}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Storage <strong class="block text-admin-ink">{project.retainedStorage}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Alerts <strong class="block text-admin-ink" data-testid="identity-project-alerts">{project.alerts}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Policy <strong class="block text-admin-ink">{project.policy}</strong></span>
              </div>
            </article>
          {/each}
        </div>
      {/if}
    </section>

    <section class="grid min-w-0 gap-3" aria-label="Service principal registry" data-testid="identity-service-principal-list">
      {#if servicePrincipalFeedback}
        <AdminMessage
          variant={servicePrincipalFeedback.variant}
          title={servicePrincipalFeedback.title}
          message={servicePrincipalFeedback.message}
          testId="identity-service-principal-message"
          compact={true}
          onDismiss={() => { servicePrincipalFeedback = null; }}
        />
      {/if}

      <section class="grid min-w-0 gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Service principal selection">
        <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
          <div class="min-w-0">
            <p class="admin-eyebrow mb-1">Service principals</p>
            <p class="m-0 text-sm font-bold text-admin-ink/72">
              {servicePrincipalRows.length} of {viewModel.servicePrincipals.length} visible principals
            </p>
          </div>
          <div class="flex flex-wrap gap-2">
            <button
              class="admin-button-primary inline-flex items-center gap-2"
              type="button"
              data-testid="identity-service-principal-new"
              disabled={disabled || !onCreateServicePrincipal}
              onclick={openCreateServicePrincipal}
            >
              <Plus size={15} aria-hidden="true" />
              New principal
            </button>
            <button
              class="admin-button-ghost inline-flex items-center gap-2"
              type="button"
              data-testid="identity-refresh-service-principals"
              disabled={disabled}
              onclick={onRefresh}
            >
              <RefreshCw size={15} class={loading ? 'animate-spin' : ''} aria-hidden="true" />
              Refresh
            </button>
          </div>
        </div>

        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Search
          <input
            class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="identity-service-principal-search"
            placeholder="Name, client id, issuer, state"
            bind:value={servicePrincipalSearch}
          />
        </label>

        {#if servicePrincipalRows.length === 0}
          <AdminMessage variant="empty" message="No service principals match the current filter." compact={true} />
        {:else}
          <div class="grid max-h-[min(300px,36vh)] min-w-0 gap-1 overflow-y-auto pr-1" aria-label="Visible service principals">
            {#each servicePrincipalRows as servicePrincipal (servicePrincipal.id)}
              <button
                class={`grid w-full min-w-0 cursor-pointer grid-cols-[4px_minmax(0,1fr)_auto] items-center gap-3 rounded-xl border p-2 text-left text-admin-ink/78 hover:border-admin-leaf/42 hover:bg-admin-field/84 ${servicePrincipalRowClass(servicePrincipal.state, servicePrincipal.id === selectedServicePrincipalId)}`}
                type="button"
                data-testid="identity-service-principal-row"
                data-service-principal-id={servicePrincipal.id}
                aria-pressed={servicePrincipal.id === selectedServicePrincipalId}
                onclick={() => selectServicePrincipal(servicePrincipal.id)}
              >
                <span class={`h-full min-h-12 rounded-full ${mappingAccentClass(servicePrincipal.state, servicePrincipal.id === selectedServicePrincipalId)}`}></span>
                <span class="grid min-w-0 gap-1">
                  <span class="flex min-w-0 items-center gap-2">
                    <strong class="min-w-0 truncate text-sm text-admin-ink" title={servicePrincipal.name}>{servicePrincipal.name}</strong>
                    {#if servicePrincipal.id === selectedServicePrincipalId}
                      <span class="rounded-full bg-admin-leaf/14 px-2 py-0.5 text-[0.68rem] font-extrabold text-admin-leaf">selected</span>
                    {/if}
                  </span>
                  <span class="min-w-0 truncate text-xs text-admin-ink/52">
                    {servicePrincipal.clientId} | {servicePrincipal.delegatedSummary}
                  </span>
                </span>
                <span class="grid justify-items-end text-xs text-[#c1d0e8]">
                  <span class="rounded-lg bg-admin-field/72 px-2 py-1">{servicePrincipal.state}</span>
                </span>
              </button>
            {/each}
          </div>
        {/if}
      </section>

      <section class="grid gap-3 rounded-[16px] border border-admin-leaf/25 bg-admin-leaf/10 p-3" aria-label="Selected service principal">
        <div class="flex min-w-0 flex-wrap items-start justify-between gap-2">
          <div class="min-w-0">
            <p class="admin-eyebrow mb-1">Selected principal</p>
            <h3 class="m-0 truncate text-base font-bold text-admin-ink" data-testid="identity-service-principal-selected-name" title={selectedServicePrincipal?.name ?? ''}>
              {selectedServicePrincipal?.name ?? 'No principal selected'}
            </h3>
          </div>
          <span class={`rounded-full border px-3 py-1 text-xs font-extrabold ${
            !selectedServicePrincipal
              ? 'border-[#90a6cc]/28 bg-admin-field/72 text-admin-ink/68'
              : selectedServicePrincipal.state === 'active'
                ? 'border-admin-leaf/30 bg-admin-leaf/12 text-admin-leaf'
                : 'border-admin-danger/32 bg-admin-danger/10 text-admin-danger'
          }`}>
            {selectedServicePrincipal?.state ?? 'select'}
          </span>
        </div>

        {#if selectedServicePrincipal}
          <div class="grid min-w-0 gap-2 text-xs text-admin-ink/70">
            {@render MappingFact('Client id', selectedServicePrincipal.client_id, 'identity-service-principal-selected-client-id')}
            {@render MappingFact('Projects', selectedServicePrincipalRow?.projects ?? 'all projects metadata unset', 'identity-service-principal-selected-projects')}
            {@render MappingFact('Delegated', selectedServicePrincipalRow?.delegatedSummary ?? '0/0 active')}
            {@render MappingFact('Scopes', selectedServicePrincipalRow?.scopes ?? 'no scopes')}
            {@render MappingFact('Updated', selectedServicePrincipal.updated_at)}
          </div>
          <p class="m-0 min-w-0 text-sm leading-normal text-admin-ink/64 [overflow-wrap:anywhere]">
            {selectedServicePrincipal.issuer}
          </p>
        {:else}
          <p class="m-0 text-sm leading-normal text-admin-ink/68">
            Select a service principal before editing or changing its lifecycle state.
          </p>
        {/if}

        <div class="flex flex-wrap gap-2">
          <button
            class="admin-button-primary inline-flex items-center gap-2"
            type="button"
            data-testid="identity-service-principal-edit"
            disabled={!selectedServicePrincipal || disabled || !onUpdateServicePrincipal}
            onclick={editSelectedServicePrincipal}
          >
            <Pencil size={15} aria-hidden="true" />
            Edit
          </button>
          <button
            class="admin-button-primary inline-flex items-center gap-2"
            type="button"
            data-testid="identity-service-principal-disable"
            disabled={!selectedServicePrincipal || selectedServicePrincipal.state === 'disabled' || disabled || !onUpdateServicePrincipal}
            onclick={() => void setSelectedServicePrincipalState('disabled')}
          >
            <ShieldOff size={15} aria-hidden="true" />
            Disable
          </button>
          <button
            class="admin-button-primary inline-flex items-center gap-2"
            type="button"
            data-testid="identity-service-principal-enable"
            disabled={!selectedServicePrincipal || selectedServicePrincipal.state === 'active' || disabled || !onUpdateServicePrincipal}
            onclick={() => void setSelectedServicePrincipalState('active')}
          >
            <ShieldCheck size={15} aria-hidden="true" />
            Enable
          </button>
        </div>
      </section>

      {#if servicePrincipalEditorOpen}
        <section class="grid min-w-0 gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Service principal editor" bind:this={servicePrincipalEditorSection}>
          <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
            <div class="min-w-0">
              <p class="admin-eyebrow mb-1">Principal editor</p>
              <h4 class="m-0 text-sm font-bold text-admin-ink">{servicePrincipalEditorTitle}</h4>
            </div>
            <button class="admin-button-ghost" type="button" disabled={disabled} onclick={closeServicePrincipalEditor}>
              Cancel
            </button>
          </div>

          <form class="grid min-w-0 gap-3" onsubmit={(event) => { event.preventDefault(); void saveServicePrincipal(); }}>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Name
              <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-service-principal-name" bind:this={servicePrincipalNameInput} bind:value={servicePrincipalForm.name} disabled={disabled} />
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Client id
              <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-service-principal-client-id" bind:value={servicePrincipalForm.clientId} disabled={disabled} />
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Issuer
              <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-service-principal-issuer" bind:value={servicePrincipalForm.issuer} disabled={disabled} />
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              State
              <select class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-service-principal-state" bind:value={servicePrincipalForm.state} disabled={disabled}>
                <option value="active">active</option>
                <option value="disabled">disabled</option>
              </select>
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Allowed projects
              <select class="min-h-28 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-2 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-service-principal-projects" multiple disabled={disabled} onchange={setAllowedProjects}>
                {#each projects as project (project.id)}
                  <option value={project.id} selected={servicePrincipalForm.allowedProjectIds.includes(project.id)} disabled={project.state === 'archived'}>{project.name} / {project.state}</option>
                {/each}
              </select>
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Description
              <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-service-principal-description" bind:value={servicePrincipalForm.description} disabled={disabled} />
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Scopes
              <textarea class="min-h-20 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field p-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-service-principal-scopes" placeholder="session:create&#10;session:delegate" bind:value={servicePrincipalForm.scopes} disabled={disabled}></textarea>
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Labels
              <textarea class="min-h-20 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field p-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-service-principal-labels" placeholder="system=mcp" bind:value={servicePrincipalForm.labels} disabled={disabled}></textarea>
            </label>

            <button
              class="admin-button-primary inline-flex w-fit items-center gap-2"
              type="submit"
              data-testid="identity-service-principal-save"
              disabled={disabled}
            >
              {#if servicePrincipalMutating}
                <RefreshCw class="animate-spin" size={15} aria-hidden="true" />
                Saving
              {:else}
                <KeyRound size={15} aria-hidden="true" />
                {servicePrincipalSaveLabel}
              {/if}
            </button>
          </form>
        </section>
      {/if}
    </section>

    <section class="grid min-w-0 gap-3" aria-label="Identity project mappings" data-testid="identity-mapping-list">
      {#if mappingFeedback}
        <AdminMessage
          variant={mappingFeedback.variant}
          title={mappingFeedback.title}
          message={mappingFeedback.message}
          testId="identity-mapping-message"
          compact={true}
          onDismiss={() => { mappingFeedback = null; }}
        />
      {/if}

      <section class="grid min-w-0 gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Identity mapping selection">
        <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
          <div class="min-w-0">
            <p class="admin-eyebrow mb-1">Identity mappings</p>
            <p class="m-0 text-sm font-bold text-admin-ink/72">
              {mappingRows.length} of {viewModel.mappings.length} visible mappings
            </p>
          </div>
          <div class="flex flex-wrap gap-2">
            <button
              class="admin-button-primary inline-flex items-center gap-2"
              type="button"
              data-testid="identity-mapping-new"
              disabled={disabled || !onCreateMapping || projects.length === 0}
              onclick={openCreateMapping}
            >
              <Plus size={15} aria-hidden="true" />
              New mapping
            </button>
            <button
              class="admin-button-ghost inline-flex items-center gap-2"
              type="button"
              data-testid="identity-refresh-mappings"
              disabled={disabled}
              onclick={onRefresh}
            >
              <RefreshCw size={15} class={loading ? 'animate-spin' : ''} aria-hidden="true" />
              Refresh
            </button>
          </div>
        </div>

        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Search
          <input
            class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="identity-mapping-search"
            placeholder="Name, kind, external id, state"
            bind:value={mappingSearch}
          />
        </label>

        {#if mappingRows.length === 0}
          <AdminMessage variant="empty" message="No identity-to-project mappings match the current filter." compact={true} />
        {:else}
          <div class="grid max-h-[min(360px,42vh)] min-w-0 gap-1 overflow-y-auto pr-1" aria-label="Visible identity mappings">
            {#each mappingRows as mapping (mapping.id)}
              <button
                class={`grid w-full min-w-0 cursor-pointer grid-cols-[4px_minmax(0,1fr)_auto] items-center gap-3 rounded-xl border p-2 text-left text-admin-ink/78 hover:border-admin-leaf/42 hover:bg-admin-field/84 ${mappingRowClass(mapping.state, mapping.id === selectedMappingId)}`}
                type="button"
                data-testid="identity-mapping-row"
                data-mapping-id={mapping.id}
                aria-pressed={mapping.id === selectedMappingId}
                onclick={() => selectMapping(mapping.id)}
              >
                <span class={`h-full min-h-12 rounded-full ${mappingAccentClass(mapping.state, mapping.id === selectedMappingId)}`}></span>
                <span class="grid min-w-0 gap-1">
                  <span class="flex min-w-0 items-center gap-2">
                    <strong class="min-w-0 truncate text-sm text-admin-ink" title={mapping.name}>{mapping.name}</strong>
                    {#if mapping.id === selectedMappingId}
                      <span class="rounded-full bg-admin-leaf/14 px-2 py-0.5 text-[0.68rem] font-extrabold text-admin-leaf">selected</span>
                    {/if}
                  </span>
                  <span class="min-w-0 truncate text-xs text-admin-ink/52">
                    {mapping.kind} | {mapping.externalIdentity} | {mapping.effective}
                  </span>
                </span>
                <span class="grid justify-items-end text-xs text-[#c1d0e8]">
                  <span class="rounded-lg bg-admin-field/72 px-2 py-1">{mapping.state}</span>
                </span>
              </button>
            {/each}
          </div>
        {/if}
      </section>

      <section class="grid gap-3 rounded-[16px] border border-admin-leaf/25 bg-admin-leaf/10 p-3" aria-label="Selected identity mapping">
        <div class="flex min-w-0 flex-wrap items-start justify-between gap-2">
          <div class="min-w-0">
            <p class="admin-eyebrow mb-1">Selected mapping</p>
            <h3 class="m-0 truncate text-base font-bold text-admin-ink" data-testid="identity-mapping-selected-name" title={selectedMapping?.name ?? ''}>
              {selectedMapping?.name ?? 'No mapping selected'}
            </h3>
          </div>
          <span class={`rounded-full border px-3 py-1 text-xs font-extrabold ${
            !selectedMapping
              ? 'border-[#90a6cc]/28 bg-admin-field/72 text-admin-ink/68'
              : selectedMapping.state === 'active'
                ? 'border-admin-leaf/30 bg-admin-leaf/12 text-admin-leaf'
                : 'border-admin-danger/32 bg-admin-danger/10 text-admin-danger'
          }`}>
            {selectedMapping?.state ?? 'select'}
          </span>
        </div>

        {#if selectedMapping}
          <div class="grid min-w-0 gap-2 text-xs text-admin-ink/70">
            {@render MappingFact('Kind', selectedMappingRow?.kind ?? selectedMapping.kind)}
            {@render MappingFact('External id', selectedMappingRow?.externalIdentity ?? selectedMapping.external_id, 'identity-mapping-selected-external-id')}
            {@render MappingFact('Project', selectedMappingRow?.projectId ?? selectedMapping.project_id, 'identity-mapping-selected-project-id')}
            {@render MappingFact('Principal', selectedMappingRow?.effective ?? 'not effective', 'identity-mapping-selected-effective')}
            {@render MappingFact('Scopes', selectedMappingRow?.scopes ?? 'no scopes')}
            {@render MappingFact('Updated', selectedMapping.updated_at)}
          </div>
          <p class="m-0 min-w-0 text-sm leading-normal text-admin-ink/64 [overflow-wrap:anywhere]">
            {selectedMapping.issuer}
          </p>
        {:else}
          <p class="m-0 text-sm leading-normal text-admin-ink/68">
            Select an identity mapping before editing or changing its lifecycle state.
          </p>
        {/if}

        <div class="flex flex-wrap gap-2">
          <button
            class="admin-button-primary inline-flex items-center gap-2"
            type="button"
            data-testid="identity-mapping-edit"
            disabled={!selectedMapping || disabled || !onUpdateMapping}
            onclick={editSelectedMapping}
          >
            <Pencil size={15} aria-hidden="true" />
            Edit
          </button>
          <button
            class="admin-button-primary inline-flex items-center gap-2"
            type="button"
            data-testid="identity-mapping-disable"
            disabled={!selectedMapping || selectedMapping.state === 'disabled' || disabled || !onUpdateMapping}
            onclick={() => void setSelectedMappingState('disabled')}
          >
            <ShieldOff size={15} aria-hidden="true" />
            Disable
          </button>
          <button
            class="admin-button-primary inline-flex items-center gap-2"
            type="button"
            data-testid="identity-mapping-enable"
            disabled={!selectedMapping || selectedMapping.state === 'active' || disabled || !onUpdateMapping}
            onclick={() => void setSelectedMappingState('active')}
          >
            <ShieldCheck size={15} aria-hidden="true" />
            Enable
          </button>
        </div>
      </section>

      {#if mappingEditorOpen}
        <section class="grid min-w-0 gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Identity mapping editor" bind:this={editorSection}>
          <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
            <div class="min-w-0">
              <p class="admin-eyebrow mb-1">Mapping editor</p>
              <h4 class="m-0 text-sm font-bold text-admin-ink">{mappingEditorTitle}</h4>
            </div>
            <button class="admin-button-ghost" type="button" disabled={disabled} onclick={closeMappingEditor}>
              Cancel
            </button>
          </div>

          <form class="grid min-w-0 gap-3" onsubmit={(event) => { event.preventDefault(); void saveMapping(); }}>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Name
              <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-mapping-name" bind:this={mappingNameInput} bind:value={mappingForm.name} disabled={disabled} />
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Kind
              <select class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-mapping-kind" value={mappingForm.kind} disabled={disabled} onchange={setMappingKind}>
                <option value="user">user</option>
                <option value="group">group</option>
                <option value="claim">claim</option>
                <option value="service_principal">service_principal</option>
              </select>
            </label>
            {#if mappingForm.kind === 'service_principal'}
              <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
                Service principal
                <select class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-mapping-service-principal-id" value={mappingForm.servicePrincipalId} disabled={disabled} onchange={setServicePrincipal}>
                  <option value="">Select service principal</option>
                  {#each servicePrincipals as servicePrincipal (servicePrincipal.id)}
                    <option value={servicePrincipal.id}>{servicePrincipal.name} / {servicePrincipal.client_id}</option>
                  {/each}
                </select>
              </label>
            {/if}
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Issuer
              <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-mapping-issuer" bind:value={mappingForm.issuer} disabled={disabled || mappingForm.kind === 'service_principal'} />
            </label>
            {#if mappingForm.kind === 'claim'}
              <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
                Claim name
                <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-mapping-claim-name" placeholder="groups" bind:value={mappingForm.claimName} disabled={disabled} />
              </label>
            {/if}
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              External identity
              <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-mapping-external-id" bind:value={mappingForm.externalId} disabled={disabled || mappingForm.kind === 'service_principal'} />
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Project
              <select class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-mapping-project-id" bind:value={mappingForm.projectId} disabled={disabled}>
                <option value="">Select project</option>
                {#each projects as project (project.id)}
                  <option value={project.id} disabled={project.state === 'archived'}>{project.name} / {project.state}</option>
                {/each}
              </select>
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              State
              <select class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-mapping-state" bind:value={mappingForm.state} disabled={disabled}>
                <option value="active">active</option>
                <option value="disabled">disabled</option>
              </select>
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Description
              <input class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-mapping-description" bind:value={mappingForm.description} disabled={disabled} />
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Scopes
              <textarea class="min-h-20 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field p-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-mapping-scopes" placeholder="session:create&#10;session:delegate" bind:value={mappingForm.scopes} disabled={disabled}></textarea>
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Labels
              <textarea class="min-h-20 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field p-3 text-admin-ink outline-none focus:border-admin-leaf/45" data-testid="identity-mapping-labels" placeholder="team=support" bind:value={mappingForm.labels} disabled={disabled}></textarea>
            </label>

            <button
              class="admin-button-primary inline-flex w-fit items-center gap-2"
              type="submit"
              data-testid="identity-mapping-save"
              disabled={disabled}
            >
              {#if mappingMutating}
                <RefreshCw class="animate-spin" size={15} aria-hidden="true" />
                Saving
              {:else}
                <ShieldCheck size={15} aria-hidden="true" />
                {mappingSaveLabel}
              {/if}
            </button>
          </form>
        </section>
      {/if}
    </section>

    {#if viewModel.unmappedSignals.length > 0}
      <section class="grid min-w-0 gap-2" aria-label="Unmapped identity signals" data-testid="identity-unmapped-signal-list">
        <div class="flex items-center justify-between gap-2">
          <p class="admin-eyebrow m-0">Unmapped signals</p>
          <span class="text-xs font-bold text-admin-ink/58">{viewModel.unmappedSignals.length}</span>
        </div>
        <div class="grid gap-2">
          {#each viewModel.unmappedSignals as signal (signal.key)}
            <article class="rounded-[12px] border border-admin-warm/24 bg-admin-warm/10 p-3">
              <strong class="block truncate text-sm font-extrabold text-admin-ink">{signal.displayName}</strong>
              <p class="m-0 min-w-0 text-xs font-semibold leading-normal text-admin-ink/58 [overflow-wrap:anywhere]">
                {signal.kind} / {signal.externalId} / {signal.issuer}
              </p>
              <p class="mt-2 mb-0 text-xs font-bold text-admin-ink/72">{signal.reason}</p>
            </article>
          {/each}
        </div>
      </section>
    {/if}

    <section class="grid min-w-0 gap-2" aria-label="Delegated automation access" data-testid="identity-delegation-list">
      <div class="flex items-center justify-between gap-2">
        <p class="admin-eyebrow m-0">Delegated automation</p>
        <span class="text-xs font-bold text-admin-ink/58">{viewModel.delegations.length}</span>
      </div>
      {#if viewModel.delegations.length === 0}
        <AdminMessage variant="empty" message="No delegated automation principals are assigned to visible sessions." compact={true} />
      {:else}
        <div class="grid gap-2">
          {#each viewModel.delegations as delegation (delegation.clientId + delegation.issuer)}
            <article class="rounded-[12px] border border-[#90a6cc]/16 bg-admin-panel/62 p-3">
              <strong class="block truncate text-sm font-extrabold text-admin-ink">{delegation.displayName}</strong>
              <p class="m-0 min-w-0 text-xs font-semibold leading-normal text-admin-ink/58 [overflow-wrap:anywhere]">
                {delegation.clientId} / {delegation.issuer}
              </p>
              <p class="mt-2 mb-0 text-xs font-bold text-admin-ink/72">
                {delegation.sessionSummary} / {delegation.registration} / {delegation.state}
                <span class="block font-semibold text-admin-ink/54">{delegation.sessionIds}</span>
              </p>
            </article>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</section>

{#snippet MappingFact(label: string, value: string, testId?: string)}
  <span class="flex min-w-0 items-center justify-between gap-3 rounded-xl bg-admin-field/72 p-2 font-bold uppercase">
    <span class="min-w-0 truncate">{label}</span>
    <strong class="min-w-0 truncate text-right font-mono text-admin-ink normal-case" data-testid={testId} title={value}>{value}</strong>
  </span>
{/snippet}
