<script lang="ts">
  import { base } from '$app/paths';
  import type {
    BrowserContextResource,
    CreateBrowserContextCommand,
    CreateSessionCommand,
    EgressProfileResource,
    ProjectResource,
    SessionTemplateResource,
  } from '../api/control-types';
  import {
    SESSION_BROWSER_CONTEXT_MODES,
    SESSION_CREATE_OWNER_MODES,
    DEFAULT_SESSION_CREATE_OWNER_MODE,
    browserContextOptionLabel,
    defaultSessionCreateFormState,
    egressProfileOptionLabel,
    egressProfileKind,
    isLocalProxyEgressPreset,
    isLocalTlsInterceptorEgressPreset,
    networkIdentitySummary,
    projectOptionLabel,
    projectUsageSummary,
    sessionBrowserContextSummary,
    sessionTemplateDefaultsSummary,
    validateBrowserContextCreateForm,
    validateSessionCreateForm,
  } from './session-create-configurator';
  import AdminMessage from './AdminMessage.svelte';

  type SessionCreateConfiguratorProps = {
    readonly onCreateSession: (command: CreateSessionCommand) => void;
    readonly onCreateBrowserContext?: (command: CreateBrowserContextCommand) => Promise<BrowserContextResource | void>;
    readonly sessionTemplates?: readonly SessionTemplateResource[];
    readonly projects?: readonly ProjectResource[];
    readonly browserContexts?: readonly BrowserContextResource[];
    readonly egressProfiles?: readonly EgressProfileResource[];
    readonly templatesLoading?: boolean;
    readonly projectsLoading?: boolean;
    readonly browserContextsLoading?: boolean;
    readonly egressProfilesLoading?: boolean;
    readonly templateError?: string | null;
    readonly projectError?: string | null;
    readonly browserContextError?: string | null;
    readonly egressProfileError?: string | null;
    readonly loading?: boolean;
    readonly disabled?: boolean;
    readonly submitTestId?: string;
    readonly submitLabel?: string;
    readonly variant?: 'panel' | 'inline';
    readonly showFileWorkspaceLink?: boolean;
    readonly payloadInitiallyOpen?: boolean;
    readonly payloadOpen?: boolean;
    readonly onPayloadOpenChange?: (open: boolean) => void;
  };

  let {
    onCreateSession,
    onCreateBrowserContext,
    sessionTemplates = [],
    projects = [],
    browserContexts = [],
    egressProfiles = [],
    templatesLoading = false,
    projectsLoading = false,
    browserContextsLoading = false,
    egressProfilesLoading = false,
    templateError = null,
    projectError = null,
    browserContextError = null,
    egressProfileError = null,
    loading = false,
    disabled = false,
    submitTestId = 'session-new',
    submitLabel = 'Create session',
    variant = 'inline',
    showFileWorkspaceLink = true,
    payloadInitiallyOpen = false,
    payloadOpen: controlledPayloadOpen,
    onPayloadOpenChange,
  }: SessionCreateConfiguratorProps = $props();

  const defaults = defaultSessionCreateFormState();
  let projectId = $state(defaults.projectId);
  let templateId = $state(defaults.templateId);
  let ownerMode = $state(defaults.ownerMode);
  let idleTimeoutSec = $state(defaults.idleTimeoutSec);
  let labels = $state(defaults.labels);
  let locale = $state(defaults.locale ?? '');
  let languages = $state(defaults.languages ?? '');
  let timezone = $state(defaults.timezone ?? '');
  let geolocationLatitude = $state(defaults.geolocationLatitude ?? '');
  let geolocationLongitude = $state(defaults.geolocationLongitude ?? '');
  let geolocationAccuracyMeters = $state(defaults.geolocationAccuracyMeters ?? '');
  let userAgent = $state(defaults.userAgent ?? '');
  let browserIdentity = $state(defaults.browserIdentity ?? '');
  let egressProfileId = $state(defaults.egressProfileId ?? '');
  let browserContextMode = $state(defaults.browserContextMode ?? 'fresh');
  let browserContextId = $state(defaults.browserContextId ?? '');
  let browserContextName = $state('');
  let browserContextLabels = $state('');
  let browserContextRetentionDays = $state('');
  let browserContextMaxProfileMb = $state('');
  let browserContextCreateError = $state<string | null>(null);
  let browserContextCreateTouched = $state(false);
  let creatingBrowserContext = $state(false);
  let justCreatedBrowserContextId = $state<string | null>(null);
  let payloadOpenInternal = $state(false);
  let previousTemplateId = $state(defaults.templateId);
  let ownerModeTouched = $state(false);
  const reusableBrowserContexts = $derived(browserContexts.filter((context) => context.persistence_mode === 'reusable'));
  const selectedBrowserContext = $derived(reusableBrowserContexts.find((context) => context.id === browserContextId) ?? null);
  const browserContextCreateValidation = $derived(validateBrowserContextCreateForm({
    projectId,
    projects,
    name: browserContextName,
    labels: browserContextLabels,
    retentionDays: browserContextRetentionDays,
    maxProfileStorageMb: browserContextMaxProfileMb,
  }));
  const browserContextCreateActive = $derived(Boolean(
    browserContextName.trim()
    || browserContextLabels.trim()
    || browserContextRetentionDays.trim()
    || browserContextMaxProfileMb.trim(),
  ));
  const validation = $derived(validateSessionCreateForm({
    projectId,
    templateId,
    ownerMode,
    idleTimeoutSec,
    labels,
    locale,
    languages,
    timezone,
    geolocationLatitude,
    geolocationLongitude,
    geolocationAccuracyMeters,
    userAgent,
    browserIdentity,
    egressProfileId,
    browserContextMode,
    browserContextId,
    browserContexts,
    egressProfiles,
    projects,
  }));
  const selectedProject = $derived(projects.find((project) => project.id === projectId) ?? null);
  const selectedTemplate = $derived(sessionTemplates.find((template) => template.id === templateId) ?? null);
  const selectedTemplateSummary = $derived(sessionTemplateDefaultsSummary(selectedTemplate));
  const selectedBrowserContextSummary = $derived(sessionBrowserContextSummary(
    browserContextMode,
    selectedBrowserContext,
  ));
  const selectedNetworkIdentitySummary = $derived(networkIdentitySummary(
    validation.command?.network_identity ?? null,
    egressProfiles,
  ));
  const rootClass = $derived(variant === 'panel'
    ? 'admin-panel mt-0 grid min-w-0 gap-4'
    : 'grid min-w-0 gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3');
  const fieldGridClass = $derived(variant === 'panel'
    ? 'grid min-w-0 gap-3 xl:grid-cols-[minmax(220px,1.2fr)_minmax(180px,1fr)_minmax(180px,1fr)_minmax(180px,1fr)]'
    : 'grid min-w-0 gap-3');
  const payloadOpen = $derived(controlledPayloadOpen ?? payloadOpenInternal);
  const proxyEgressProfiles = $derived(egressProfiles.filter((profile) => egressProfileKind(profile) === 'proxy'));
  const tlsEgressProfiles = $derived(egressProfiles.filter((profile) => egressProfileKind(profile) === 'tls_interceptor'));
  const otherEgressProfiles = $derived(egressProfiles.filter((profile) => egressProfileKind(profile) === 'other'));
  const localProxyEgressPreset = $derived(proxyEgressProfiles.find(isLocalProxyEgressPreset) ?? null);
  const localTlsEgressPreset = $derived(tlsEgressProfiles.find(isLocalTlsInterceptorEgressPreset) ?? null);
  const additionalProxyEgressProfiles = $derived(
    proxyEgressProfiles.filter((profile) => profile.id !== localProxyEgressPreset?.id),
  );
  const additionalTlsEgressProfiles = $derived(
    tlsEgressProfiles.filter((profile) => profile.id !== localTlsEgressPreset?.id),
  );

  $effect(() => {
    if (payloadInitiallyOpen) {
      payloadOpenInternal = true;
    }
  });

  $effect(() => {
    if (
      projectId
      && !projectsLoading
      && projects.length > 0
      && !projects.some((project) => project.id === projectId)
    ) {
      projectId = '';
    }
  });

  $effect(() => {
    if (templateId && !sessionTemplates.some((template) => template.id === templateId)) {
      templateId = '';
    }
  });

  $effect(() => {
    if (
      egressProfileId
      && !egressProfilesLoading
      && egressProfiles.length > 0
      && !egressProfiles.some((profile) => profile.id === egressProfileId)
    ) {
      egressProfileId = '';
    }
  });

  $effect(() => {
    if (
      browserContextId
      && !browserContextsLoading
      && browserContexts.length > 0
      && !browserContexts.some((context) => context.id === browserContextId)
    ) {
      if (justCreatedBrowserContextId === browserContextId) {
        return;
      }
      browserContextId = '';
    }
    if (
      justCreatedBrowserContextId
      && browserContexts.some((context) => context.id === justCreatedBrowserContextId)
    ) {
      if (browserContextMode === 'reusable') {
        browserContextId = justCreatedBrowserContextId;
      }
      justCreatedBrowserContextId = null;
    }
  });

  $effect(() => {
    if (browserContextMode !== 'reusable' && browserContextId) {
      browserContextId = '';
    }
  });

  $effect(() => {
    if (templateId === previousTemplateId) {
      return;
    }
    if (templateId && !ownerModeTouched && ownerMode === DEFAULT_SESSION_CREATE_OWNER_MODE) {
      ownerMode = '';
    } else if (!templateId && !ownerMode) {
      ownerMode = DEFAULT_SESSION_CREATE_OWNER_MODE;
      ownerModeTouched = false;
    }
    previousTemplateId = templateId;
  });

  function submit(): void {
    if (!validation.command) {
      return;
    }
    onCreateSession(validation.command);
  }

  function egressProfileOptionDisabled(profile: EgressProfileResource): boolean {
    return profile.state === 'disabled' || Boolean(profile.project_id && profile.project_id !== projectId);
  }

  function setPayloadOpen(open: boolean): void {
    payloadOpenInternal = open;
    onPayloadOpenChange?.(open);
  }

  async function submitBrowserContextCreate(): Promise<void> {
    browserContextCreateError = null;
    browserContextCreateTouched = true;
    if (!onCreateBrowserContext || !browserContextCreateValidation.command) {
      return;
    }
    creatingBrowserContext = true;
    try {
      const created = await onCreateBrowserContext(browserContextCreateValidation.command);
      browserContextName = '';
      browserContextLabels = '';
      browserContextRetentionDays = '';
      browserContextMaxProfileMb = '';
      browserContextCreateTouched = false;
      if (created?.id) {
        justCreatedBrowserContextId = created.id;
        browserContextMode = 'reusable';
        browserContextId = created.id;
      }
    } catch (error) {
      browserContextCreateError = error instanceof Error
        ? error.message
        : 'Browser context creation failed.';
    } finally {
      creatingBrowserContext = false;
    }
  }
</script>

<section class={rootClass} aria-label="Create session" data-testid="session-create-configurator">
  <div class="flex min-w-0 flex-wrap items-start justify-between gap-3">
    <div class="min-w-0">
      <p class="admin-eyebrow mb-1">Create session</p>
      <h3 class="m-0 text-base font-bold text-admin-ink">Session configuration</h3>
    </div>
    {#if showFileWorkspaceLink}
      <div class="flex flex-wrap gap-2">
        <a class="admin-button-ghost min-h-9 px-3 text-sm" href={`${base}/browser-contexts`}>
          Context catalog
        </a>
        <a class="admin-button-ghost min-h-9 px-3 text-sm" href={`${base}/files/workspaces`}>
          File workspaces
        </a>
      </div>
    {/if}
  </div>

  <form
    class="grid gap-3"
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <div class={fieldGridClass}>
      <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
        Project
        <select
          class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="session-create-project"
          bind:value={projectId}
          disabled={loading || disabled || projectsLoading}
        >
          <option value="">Owner scope</option>
          {#each projects as project}
            <option value={project.id} disabled={project.state === 'archived'}>
              {projectOptionLabel(project)}
            </option>
          {/each}
        </select>
      </label>

      <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
        Template
        <select
          class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="session-create-template"
          bind:value={templateId}
          disabled={loading || disabled || templatesLoading}
        >
          <option value="">No template</option>
          {#each sessionTemplates as template}
            <option value={template.id}>{template.name}</option>
          {/each}
        </select>
      </label>

      <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
        Owner mode
        <select
          class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="session-create-owner-mode"
          bind:value={ownerMode}
          disabled={loading || disabled}
          onchange={() => { ownerModeTouched = true; }}
        >
          <option value="">Template / backend default</option>
          {#each SESSION_CREATE_OWNER_MODES as mode}
            <option value={mode.value}>{mode.label}</option>
          {/each}
        </select>
      </label>

      <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
        Idle timeout
        <input
          class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="session-create-idle-timeout"
          inputmode="numeric"
          placeholder="Backend default"
          type="text"
          bind:value={idleTimeoutSec}
          disabled={loading || disabled}
        />
      </label>

      <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
        Browser context
        <select
          class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="session-create-browser-context-mode"
          bind:value={browserContextMode}
          disabled={loading || disabled}
        >
          {#each SESSION_BROWSER_CONTEXT_MODES as mode}
            <option value={mode.value}>{mode.label}</option>
          {/each}
        </select>
      </label>
    </div>

    {#if projectsLoading || projectError || selectedProject}
      <section
        class="grid min-w-0 gap-3 rounded-xl border border-admin-ink/10 bg-admin-field/68 p-3 text-sm text-admin-ink/72"
        aria-label="Project scope"
        data-testid="session-create-project-summary"
      >
        {#if projectsLoading}
          <span class="text-xs font-bold text-[#c1d0e8]">Loading projects...</span>
        {:else if projectError}
          <AdminMessage
            variant="warning"
            title="Project catalog unavailable"
            message={projectError}
            compact={true}
          />
        {:else if selectedProject}
          <div class="flex min-w-0 flex-wrap items-start justify-between gap-2">
            <span class="min-w-0">
              <strong class="block truncate text-admin-ink">{selectedProject.name}</strong>
              <span class="block truncate text-xs text-admin-ink/58">{selectedProject.id}</span>
            </span>
            <span class="rounded-lg bg-admin-leaf/12 px-2 py-1 text-xs font-bold text-admin-leaf">
              {selectedProject.state}
            </span>
          </div>
          <p class="m-0 text-xs leading-normal text-admin-ink/62 [overflow-wrap:anywhere]">
            {projectUsageSummary(selectedProject)}
          </p>
        {/if}
      </section>
    {/if}

    <section
      class="grid min-w-0 gap-3 rounded-xl border border-admin-ink/10 bg-admin-field/68 p-3 text-sm text-admin-ink/72"
      aria-label="Browser context"
      data-testid="session-create-browser-context-summary"
    >
      <div class="grid min-w-0 gap-3 md:grid-cols-[minmax(0,1fr)_minmax(220px,0.7fr)] md:items-end">
        <div class="min-w-0">
          <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
            <strong class="text-admin-ink">Browser profile source</strong>
            <span class="rounded-lg bg-admin-leaf/12 px-2 py-1 text-xs font-bold text-admin-leaf">
              {browserContextMode}
            </span>
          </div>
          <p class="m-0 mt-2 text-xs leading-normal text-admin-ink/62 [overflow-wrap:anywhere]">
            {selectedBrowserContextSummary}
          </p>
        </div>

        {#if browserContextMode === 'reusable'}
          <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
            Reusable context
            <select
              class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
              data-testid="session-create-browser-context-id"
              bind:value={browserContextId}
              disabled={loading || disabled || browserContextsLoading}
            >
              <option value="">Select reusable context</option>
              {#each reusableBrowserContexts as context}
                <option
                  value={context.id}
                  disabled={context.state !== 'ready' || Boolean(context.project_id && context.project_id !== projectId)}
                >
                  {browserContextOptionLabel(context)}
                </option>
              {/each}
            </select>
          </label>
        {/if}
      </div>

      {#if browserContextsLoading}
        <span class="text-xs font-bold text-[#c1d0e8]">Loading browser context catalog...</span>
      {:else if browserContextError}
        <AdminMessage
          variant="warning"
          title="Browser context catalog unavailable"
          message={browserContextError}
          compact={true}
        />
      {:else if browserContextMode === 'reusable' && reusableBrowserContexts.length === 0}
        <AdminMessage
          variant="empty"
          message="No reusable browser contexts are available for this operator."
          compact={true}
        />
      {/if}

      {#if onCreateBrowserContext}
        <div class="grid min-w-0 gap-2 rounded-xl border border-admin-ink/10 bg-admin-panel/52 p-3">
          <div class="grid min-w-0 gap-2 [grid-template-columns:repeat(auto-fit,minmax(min(100%,140px),1fr))]">
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              New context
              <input
                class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
                data-testid="session-create-context-name"
                placeholder="support-profile"
                type="text"
                bind:value={browserContextName}
                oninput={() => { browserContextCreateTouched = true; }}
                disabled={loading || disabled || creatingBrowserContext}
              />
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Context labels
              <input
                class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
                data-testid="session-create-context-labels"
                placeholder="team=support, suite=smoke"
                type="text"
                bind:value={browserContextLabels}
                oninput={() => { browserContextCreateTouched = true; }}
                disabled={loading || disabled || creatingBrowserContext}
              />
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Retention days
              <input
                class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
                data-testid="session-create-context-retention-days"
                inputmode="numeric"
                placeholder="Manual"
                type="text"
                bind:value={browserContextRetentionDays}
                oninput={() => { browserContextCreateTouched = true; }}
                disabled={loading || disabled || creatingBrowserContext}
              />
            </label>
            <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
              Max profile MB
              <input
                class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
                data-testid="session-create-context-max-profile-mb"
                inputmode="numeric"
                placeholder="No limit"
                type="text"
                bind:value={browserContextMaxProfileMb}
                oninput={() => { browserContextCreateTouched = true; }}
                disabled={loading || disabled || creatingBrowserContext}
              />
            </label>
            <button
              class="admin-button-ghost min-h-11"
              type="button"
              data-testid="session-create-context-create"
              disabled={loading || disabled || creatingBrowserContext || !browserContextCreateValidation.command}
              onclick={() => void submitBrowserContextCreate()}
            >
              {creatingBrowserContext ? 'Saving...' : 'Save context'}
            </button>
          </div>
          {#if (browserContextCreateTouched && browserContextCreateActive && browserContextCreateValidation.errors.length > 0) || browserContextCreateError}
            <AdminMessage
              variant="error"
              title="Invalid browser context"
              message={browserContextCreateError ?? browserContextCreateValidation.errors.join(' ')}
              testId="session-create-context-create-error"
              compact={true}
            />
          {/if}
        </div>
      {/if}
    </section>

    <section
      class="grid min-w-0 gap-3 rounded-xl border border-admin-ink/10 bg-admin-field/68 p-3 text-sm text-admin-ink/72"
      aria-label="Network identity"
      data-testid="session-create-network-identity-summary"
    >
      <div class="flex min-w-0 flex-wrap items-start justify-between gap-2">
        <div class="min-w-0">
          <strong class="text-admin-ink">Network identity</strong>
          <p class="m-0 mt-1 text-xs leading-normal text-admin-ink/62 [overflow-wrap:anywhere]" data-testid="session-create-network-summary">
            {selectedNetworkIdentitySummary}
          </p>
        </div>
        <span class="rounded-lg bg-admin-leaf/12 px-2 py-1 text-xs font-bold text-admin-leaf">
          Optional
        </span>
      </div>

      <div class="grid min-w-0 gap-2 [grid-template-columns:repeat(auto-fit,minmax(min(100%,170px),1fr))]">
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Locale
          <input
            class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-create-locale"
            placeholder="de-DE"
            type="text"
            bind:value={locale}
            disabled={loading || disabled}
          />
        </label>
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Languages
          <input
            class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-create-languages"
            placeholder="de-DE, en-US"
            type="text"
            bind:value={languages}
            disabled={loading || disabled}
          />
        </label>
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Timezone
          <input
            class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-create-timezone"
            placeholder="Europe/Berlin"
            type="text"
            bind:value={timezone}
            disabled={loading || disabled}
          />
        </label>
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Egress profile
          <select
            class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-create-egress-profile"
            bind:value={egressProfileId}
            disabled={loading || disabled || egressProfilesLoading}
          >
            <option value="">No egress</option>
            {#if localProxyEgressPreset}
              <option value={localProxyEgressPreset.id} disabled={egressProfileOptionDisabled(localProxyEgressPreset)}>
                Egress as Proxy
              </option>
            {/if}
            {#if localTlsEgressPreset}
              <option value={localTlsEgressPreset.id} disabled={egressProfileOptionDisabled(localTlsEgressPreset)}>
                Egress as TLS Interceptor
              </option>
            {/if}
            {#if additionalProxyEgressProfiles.length > 0}
              <optgroup label={localProxyEgressPreset ? 'Additional proxy profiles' : 'Egress as Proxy'}>
                {#each additionalProxyEgressProfiles as profile}
                  <option value={profile.id} disabled={egressProfileOptionDisabled(profile)}>
                    {egressProfileOptionLabel(profile)}
                  </option>
                {/each}
              </optgroup>
            {/if}
            {#if additionalTlsEgressProfiles.length > 0}
              <optgroup label={localTlsEgressPreset ? 'Additional TLS interceptor profiles' : 'Egress as TLS Interceptor'}>
                {#each additionalTlsEgressProfiles as profile}
                  <option value={profile.id} disabled={egressProfileOptionDisabled(profile)}>
                    {egressProfileOptionLabel(profile)}
                  </option>
                {/each}
              </optgroup>
            {/if}
            {#if otherEgressProfiles.length > 0}
              <optgroup label="Other egress profiles">
                {#each otherEgressProfiles as profile}
                  <option value={profile.id} disabled={egressProfileOptionDisabled(profile)}>
                    {egressProfileOptionLabel(profile)}
                  </option>
                {/each}
              </optgroup>
            {/if}
          </select>
        </label>
      </div>

      <div class="grid min-w-0 gap-2 [grid-template-columns:repeat(auto-fit,minmax(min(100%,150px),1fr))]">
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Latitude
          <input
            class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-create-geolocation-latitude"
            inputmode="decimal"
            placeholder="52.5200"
            type="text"
            bind:value={geolocationLatitude}
            disabled={loading || disabled}
          />
        </label>
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Longitude
          <input
            class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-create-geolocation-longitude"
            inputmode="decimal"
            placeholder="13.4050"
            type="text"
            bind:value={geolocationLongitude}
            disabled={loading || disabled}
          />
        </label>
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Accuracy meters
          <input
            class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-create-geolocation-accuracy"
            inputmode="decimal"
            placeholder="100"
            type="text"
            bind:value={geolocationAccuracyMeters}
            disabled={loading || disabled}
          />
        </label>
      </div>

      <div class="grid min-w-0 gap-2 md:grid-cols-2">
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          Browser identity
          <input
            class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-create-browser-identity"
            placeholder="desktop-chromium-stable"
            type="text"
            bind:value={browserIdentity}
            disabled={loading || disabled}
          />
        </label>
        <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
          User agent
          <input
            class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
            data-testid="session-create-user-agent"
            placeholder="Backend default"
            type="text"
            bind:value={userAgent}
            disabled={loading || disabled}
          />
        </label>
      </div>

      {#if egressProfilesLoading}
        <span class="text-xs font-bold text-[#c1d0e8]">Loading egress profiles...</span>
      {:else if egressProfileError}
        <AdminMessage
          variant="warning"
          title="Egress profiles unavailable"
          message={egressProfileError}
          compact={true}
        />
      {/if}
    </section>

    {#if selectedTemplate || templateError || templatesLoading}
      <section
        class="rounded-xl border border-admin-ink/10 bg-admin-field/68 p-3 text-sm text-admin-ink/72"
        aria-label="Selected session template"
        data-testid="session-create-template-summary"
      >
        {#if templatesLoading}
          <span class="font-bold text-[#c1d0e8]">Loading template catalog...</span>
        {:else if templateError}
          <AdminMessage
            variant="warning"
            title="Template catalog unavailable"
            message={templateError}
            compact={true}
          />
        {:else if selectedTemplate}
          <div class="flex min-w-0 flex-wrap items-start justify-between gap-2">
            <span class="min-w-0">
              <strong class="block truncate text-admin-ink">{selectedTemplate.name}</strong>
              <span class="block truncate text-xs text-admin-ink/58">{selectedTemplate.id}</span>
            </span>
            <span class="rounded-lg bg-admin-leaf/12 px-2 py-1 text-xs font-bold text-admin-leaf">
              v{selectedTemplate.version}
            </span>
          </div>
          <p class="m-0 mt-2 text-xs leading-normal text-admin-ink/62 [overflow-wrap:anywhere]">
            {selectedTemplateSummary}
          </p>
        {/if}
      </section>
    {/if}

    <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
      Labels
      <textarea
        class="min-h-20 min-w-0 resize-y rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-2 text-admin-ink outline-none focus:border-admin-leaf/45"
        data-testid="session-create-labels"
        placeholder="case=1234, purpose=import-repro"
        rows={variant === 'panel' ? 3 : 2}
        bind:value={labels}
        disabled={loading || disabled}
      ></textarea>
    </label>

    <section class="rounded-xl border border-admin-ink/10 bg-admin-field/68 p-3" aria-label="API payload preview">
      <button
        class="flex w-full cursor-pointer items-center justify-between gap-3 border-0 bg-transparent p-0 text-left text-xs font-bold uppercase text-[#c1d0e8]"
        type="button"
        aria-expanded={payloadOpen}
        data-testid="session-create-preview-toggle"
        onclick={() => setPayloadOpen(!payloadOpen)}
      >
        <span>API payload</span>
        <span aria-hidden="true">{payloadOpen ? 'Hide' : 'Show'}</span>
      </button>
      {#if payloadOpen}
        <pre class="mt-3 max-h-44 overflow-auto whitespace-pre-wrap text-xs text-admin-ink" data-testid="session-create-preview">{validation.preview}</pre>
      {/if}
    </section>

    {#if validation.errors.length > 0}
      <AdminMessage
        variant="error"
        title="Invalid session configuration"
        message={validation.errors.join(' ')}
        testId="session-create-error"
        compact={true}
      />
    {/if}

    <button
      class="admin-button-primary w-fit"
      type="submit"
      data-testid={submitTestId}
      disabled={loading || disabled || validation.errors.length > 0}
    >
      {loading ? 'Creating...' : submitLabel}
    </button>
</form>
</section>

<style>
  section[data-testid='session-create-configurator'] :where(input, select, textarea) {
    width: 100%;
    max-width: 100%;
  }

  section[data-testid='session-create-configurator'] :where(a, button) {
    max-width: 100%;
  }
</style>
