<script lang="ts">
  import { base } from '$app/paths';
  import type {
    BrowserContextResource,
    CreateBrowserContextCommand,
    CreateSessionCommand,
    SessionTemplateResource,
  } from '../api/control-types';
  import {
    SESSION_BROWSER_CONTEXT_MODES,
    SESSION_CREATE_OWNER_MODES,
    DEFAULT_SESSION_CREATE_OWNER_MODE,
    browserContextOptionLabel,
    defaultSessionCreateFormState,
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
    readonly browserContexts?: readonly BrowserContextResource[];
    readonly templatesLoading?: boolean;
    readonly browserContextsLoading?: boolean;
    readonly templateError?: string | null;
    readonly browserContextError?: string | null;
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
    browserContexts = [],
    templatesLoading = false,
    browserContextsLoading = false,
    templateError = null,
    browserContextError = null,
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
  let templateId = $state(defaults.templateId);
  let ownerMode = $state(defaults.ownerMode);
  let idleTimeoutSec = $state(defaults.idleTimeoutSec);
  let labels = $state(defaults.labels);
  let browserContextMode = $state(defaults.browserContextMode ?? 'fresh');
  let browserContextId = $state(defaults.browserContextId ?? '');
  let browserContextName = $state('');
  let browserContextLabels = $state('');
  let browserContextRetentionDays = $state('');
  let browserContextCreateError = $state<string | null>(null);
  let browserContextCreateTouched = $state(false);
  let creatingBrowserContext = $state(false);
  let payloadOpenInternal = $state(false);
  let previousTemplateId = $state(defaults.templateId);
  let ownerModeTouched = $state(false);
  const reusableBrowserContexts = $derived(browserContexts.filter((context) => context.persistence_mode === 'reusable'));
  const selectedBrowserContext = $derived(reusableBrowserContexts.find((context) => context.id === browserContextId) ?? null);
  const browserContextCreateValidation = $derived(validateBrowserContextCreateForm({
    name: browserContextName,
    labels: browserContextLabels,
    retentionDays: browserContextRetentionDays,
  }));
  const browserContextCreateActive = $derived(Boolean(
    browserContextName.trim()
    || browserContextLabels.trim()
    || browserContextRetentionDays.trim(),
  ));
  const validation = $derived(validateSessionCreateForm({
    templateId,
    ownerMode,
    idleTimeoutSec,
    labels,
    browserContextMode,
    browserContextId,
    browserContexts,
  }));
  const selectedTemplate = $derived(sessionTemplates.find((template) => template.id === templateId) ?? null);
  const selectedTemplateSummary = $derived(sessionTemplateDefaultsSummary(selectedTemplate));
  const selectedBrowserContextSummary = $derived(sessionBrowserContextSummary(
    browserContextMode,
    selectedBrowserContext,
  ));
  const rootClass = $derived(variant === 'panel'
    ? 'admin-panel mt-0 grid min-w-0 gap-4'
    : 'grid min-w-0 gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3');
  const fieldGridClass = $derived(variant === 'panel'
    ? 'grid min-w-0 gap-3 xl:grid-cols-[minmax(220px,1.2fr)_minmax(180px,1fr)_minmax(180px,1fr)_minmax(180px,1fr)]'
    : 'grid min-w-0 gap-3');
  const payloadOpen = $derived(controlledPayloadOpen ?? payloadOpenInternal);

  $effect(() => {
    if (payloadInitiallyOpen) {
      payloadOpenInternal = true;
    }
  });

  $effect(() => {
    if (templateId && !sessionTemplates.some((template) => template.id === templateId)) {
      templateId = '';
    }
  });

  $effect(() => {
    if (
      browserContextId
      && !browserContextsLoading
      && browserContexts.length > 0
      && !browserContexts.some((context) => context.id === browserContextId)
    ) {
      browserContextId = '';
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
      browserContextCreateTouched = false;
      if (created?.id) {
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
          <p class="m-0 mt-2 text-xs leading-normal text-admin-ink/62">
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
                <option value={context.id} disabled={context.state !== 'ready'}>
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
          <div class="grid min-w-0 gap-2 md:grid-cols-[minmax(160px,0.7fr)_minmax(180px,1fr)_minmax(130px,0.45fr)_auto] md:items-end">
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
          <p class="m-0 mt-2 text-xs leading-normal text-admin-ink/62">
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
