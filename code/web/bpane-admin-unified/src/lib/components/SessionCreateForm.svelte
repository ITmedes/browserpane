<script lang="ts">
  import type { CreateSessionRequest } from '$lib/sessions/session-types';
  import {
    createNewSessionCreateDraft,
    emptySessionCreateOptions,
    hasSessionCreateDraftChanges,
    validateSessionCreateDraft,
    type SessionCreateDraft,
    type SessionCreateOptionsLoadState,
  } from '$lib/sessions/session-create-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import FieldFeedback from './FieldFeedback.svelte';

  type SessionCreateFormProps = {
    readonly disabled?: boolean;
    readonly optionsState?: SessionCreateOptionsLoadState;
    readonly onSave?: (request: CreateSessionRequest) => void | Promise<void>;
  };

  let {
    disabled = false,
    optionsState = { status: 'idle' },
    onSave,
  }: SessionCreateFormProps = $props();
  // svelte-ignore state_referenced_locally
  let draft = $state<SessionCreateDraft>(createNewSessionCreateDraft());

  const options = $derived(optionsState.status === 'ready' ? optionsState.options : emptySessionCreateOptions());
  const validation = $derived(validateSessionCreateDraft(draft, options));
  const changed = $derived(hasSessionCreateDraftChanges(draft));
  const changeStatusLabel = $derived(disabled
    ? 'Read-only access'
    : validation.valid
      ? 'Ready to create'
      : 'Session draft');
  const changeStatusClass = $derived(disabled
    ? 'border-admin-border bg-admin-soft text-admin-muted'
    : validation.valid
      ? 'border-emerald-200 bg-emerald-50 text-emerald-800'
      : 'border-amber-200 bg-amber-50 text-amber-900');

  function reset(): void {
    draft = createNewSessionCreateDraft();
  }

  function save(): void {
    if (!validation.request) {
      return;
    }
    void onSave?.(validation.request);
  }
</script>

<section class="rounded-md border border-admin-border bg-admin-panel p-4 sm:p-5" data-testid="session-create-form">
  <div class="flex flex-col gap-3 border-b border-admin-border pb-4 lg:flex-row lg:items-start lg:justify-between">
    <div class="min-w-0">
      <h3 class="m-0 text-base font-semibold text-admin-ink">New session settings</h3>
      <p class="m-0 mt-1 text-sm leading-6 text-admin-muted">
        Choose the project, policy templates, browser profile, egress, and runtime metadata before creating the session.
      </p>
    </div>
    <span class={`inline-flex w-fit shrink-0 rounded-full border px-2.5 py-1 text-xs font-semibold ${changeStatusClass}`}>
      {changeStatusLabel}
    </span>
  </div>

  {#if optionsState.status === 'loading'}
    <div class="mt-4">
      <AdminMessage tone="loading" title="Loading session options" testId="session-create-options-loading" />
    </div>
  {:else if optionsState.status === 'error'}
    <div class="mt-4">
      <AdminMessage
        tone="warning"
        title="Session options unavailable"
        message={optionsState.message}
        testId="session-create-options-error"
      />
    </div>
  {/if}

  <div class="mt-5 grid gap-4">
    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="session-create-scope-section">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Scope and ownership</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Leave fields empty when the session should use gateway defaults.
        </p>
      </div>

      <div class="mt-4 grid items-start gap-4 lg:grid-cols-3">
        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Project</span>
          <select
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.projectId}
            disabled={disabled}
            data-testid="session-create-project-id"
          >
            <option value="">Owner scoped</option>
            {#each options.projects as project}
              <option value={project.id}>{project.name} - {project.state}</option>
            {/each}
          </select>
          <FieldFeedback errors={validation.fieldErrors.projectId} testId="session-create-project-id-error" />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Session template</span>
          <select
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.templateId}
            disabled={disabled}
            data-testid="session-create-template-id"
          >
            <option value="">No template</option>
            {#each options.sessionTemplates as template}
              <option value={template.id}>{template.name}{template.state ? ` - ${template.state}` : ''}</option>
            {/each}
          </select>
          <FieldFeedback errors={validation.fieldErrors.templateId} testId="session-create-template-id-error" />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Owner mode</span>
          <select
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.ownerMode}
            disabled={disabled}
            data-testid="session-create-owner-mode"
          >
            <option value="">Gateway default</option>
            <option value="collaborative">collaborative</option>
            <option value="exclusive_browser_owner">exclusive browser owner</option>
          </select>
        </label>
      </div>
    </section>

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="session-create-runtime-section">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Browser runtime references</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Select explicit browser context or egress resources only when the session should bind to them.
        </p>
      </div>

      <div class="mt-4 grid items-start gap-4 lg:grid-cols-3">
        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Browser context mode</span>
          <select
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.browserContextMode}
            disabled={disabled}
            data-testid="session-create-browser-context-mode"
          >
            <option value="">Gateway default</option>
            <option value="fresh">fresh profile</option>
            <option value="ephemeral">ephemeral profile</option>
            <option value="reusable">reusable context</option>
          </select>
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Reusable context</span>
          <select
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.browserContextId}
            disabled={disabled || draft.browserContextMode !== 'reusable'}
            data-testid="session-create-browser-context-id"
          >
            <option value="">Select context</option>
            {#each options.browserContexts as context}
              <option value={context.id}>{context.name}{context.scope ? ` - ${context.scope}` : ''}</option>
            {/each}
          </select>
          <FieldFeedback errors={validation.fieldErrors.browserContextId} testId="session-create-browser-context-id-error" />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Egress profile</span>
          <select
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.egressProfileId}
            disabled={disabled}
            data-testid="session-create-egress-profile-id"
          >
            <option value="">No explicit egress profile</option>
            {#each options.egressProfiles as profile}
              <option value={profile.id}>{profile.name}{profile.scope ? ` - ${profile.scope}` : ''}</option>
            {/each}
          </select>
          <FieldFeedback errors={validation.fieldErrors.egressProfileId} testId="session-create-egress-profile-id-error" />
        </label>
      </div>
    </section>

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="session-create-capabilities-section">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Capabilities</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Disable browser features that should not be available in this session.
        </p>
      </div>

      <div class="mt-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <label class="flex min-w-0 items-start gap-3 rounded-md border border-admin-border bg-white p-3 text-sm transition hover:border-admin-accent/50 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-admin-accent/25">
          <input
            class="mt-0.5 size-4 shrink-0 rounded border-admin-border text-admin-accent focus:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60"
            type="checkbox"
            bind:checked={draft.capabilityBrowserInput}
            disabled={disabled}
            data-testid="session-create-capability-browser-input"
          />
          <span class="min-w-0">
            <span class="block font-medium text-admin-ink">Browser input</span>
            <span class="block text-xs leading-5 text-admin-muted">Keyboard and pointer control.</span>
          </span>
        </label>

        <label class="flex min-w-0 items-start gap-3 rounded-md border border-admin-border bg-white p-3 text-sm transition hover:border-admin-accent/50 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-admin-accent/25">
          <input
            class="mt-0.5 size-4 shrink-0 rounded border-admin-border text-admin-accent focus:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60"
            type="checkbox"
            bind:checked={draft.capabilityClipboard}
            disabled={disabled}
            data-testid="session-create-capability-clipboard"
          />
          <span class="min-w-0">
            <span class="block font-medium text-admin-ink">Clipboard</span>
            <span class="block text-xs leading-5 text-admin-muted">Copy and paste access.</span>
          </span>
        </label>

        <label class="flex min-w-0 items-start gap-3 rounded-md border border-admin-border bg-white p-3 text-sm transition hover:border-admin-accent/50 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-admin-accent/25">
          <input
            class="mt-0.5 size-4 shrink-0 rounded border-admin-border text-admin-accent focus:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60"
            type="checkbox"
            bind:checked={draft.capabilityAudio}
            disabled={disabled}
            data-testid="session-create-capability-audio"
          />
          <span class="min-w-0">
            <span class="block font-medium text-admin-ink">Audio output</span>
            <span class="block text-xs leading-5 text-admin-muted">Remote browser sound.</span>
          </span>
        </label>

        <label class="flex min-w-0 items-start gap-3 rounded-md border border-admin-border bg-white p-3 text-sm transition hover:border-admin-accent/50 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-admin-accent/25">
          <input
            class="mt-0.5 size-4 shrink-0 rounded border-admin-border text-admin-accent focus:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60"
            type="checkbox"
            bind:checked={draft.capabilityMicrophone}
            disabled={disabled}
            data-testid="session-create-capability-microphone"
          />
          <span class="min-w-0">
            <span class="block font-medium text-admin-ink">Microphone</span>
            <span class="block text-xs leading-5 text-admin-muted">Local microphone ingress.</span>
          </span>
        </label>

        <label class="flex min-w-0 items-start gap-3 rounded-md border border-admin-border bg-white p-3 text-sm transition hover:border-admin-accent/50 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-admin-accent/25">
          <input
            class="mt-0.5 size-4 shrink-0 rounded border-admin-border text-admin-accent focus:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60"
            type="checkbox"
            bind:checked={draft.capabilityCamera}
            disabled={disabled}
            data-testid="session-create-capability-camera"
          />
          <span class="min-w-0">
            <span class="block font-medium text-admin-ink">Camera</span>
            <span class="block text-xs leading-5 text-admin-muted">Browser camera ingress.</span>
          </span>
        </label>

        <label class="flex min-w-0 items-start gap-3 rounded-md border border-admin-border bg-white p-3 text-sm transition hover:border-admin-accent/50 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-admin-accent/25">
          <input
            class="mt-0.5 size-4 shrink-0 rounded border-admin-border text-admin-accent focus:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60"
            type="checkbox"
            bind:checked={draft.capabilityFileTransfer}
            disabled={disabled}
            data-testid="session-create-capability-file-transfer"
          />
          <span class="min-w-0">
            <span class="block font-medium text-admin-ink">File transfer</span>
            <span class="block text-xs leading-5 text-admin-muted">Live upload and download transfer.</span>
          </span>
        </label>

        <label class="flex min-w-0 items-start gap-3 rounded-md border border-admin-border bg-white p-3 text-sm transition hover:border-admin-accent/50 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-admin-accent/25">
          <input
            class="mt-0.5 size-4 shrink-0 rounded border-admin-border text-admin-accent focus:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60"
            type="checkbox"
            bind:checked={draft.capabilityResize}
            disabled={disabled}
            data-testid="session-create-capability-resize"
          />
          <span class="min-w-0">
            <span class="block font-medium text-admin-ink">Resize</span>
            <span class="block text-xs leading-5 text-admin-muted">Viewport resize requests.</span>
          </span>
        </label>
      </div>
    </section>

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="session-create-metadata-section">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Runtime metadata</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Optional launch metadata is sent only when a value is entered.
        </p>
      </div>

      <div class="mt-4 grid items-start gap-4 lg:grid-cols-3">
        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Idle timeout seconds</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="number"
            min="1"
            bind:value={draft.idleTimeoutSec}
            disabled={disabled}
            data-testid="session-create-idle-timeout"
          />
          <FieldFeedback errors={validation.fieldErrors.idleTimeoutSec} testId="session-create-idle-timeout-error" />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Viewport width</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="number"
            min="320"
            bind:value={draft.viewportWidth}
            disabled={disabled}
            data-testid="session-create-viewport-width"
          />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Viewport height</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="number"
            min="240"
            bind:value={draft.viewportHeight}
            disabled={disabled}
            data-testid="session-create-viewport-height"
          />
          <FieldFeedback errors={validation.fieldErrors.viewport} testId="session-create-viewport-error" />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Locale</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="text"
            bind:value={draft.locale}
            disabled={disabled}
            placeholder="en-US"
            autocomplete="off"
            data-testid="session-create-locale"
          />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Languages</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="text"
            bind:value={draft.languagesText}
            disabled={disabled}
            placeholder="en-US, de-DE"
            autocomplete="off"
            data-testid="session-create-languages"
          />
          <FieldFeedback errors={validation.fieldErrors.languagesText} testId="session-create-languages-error" />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Timezone</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="text"
            bind:value={draft.timezone}
            disabled={disabled}
            placeholder="Europe/Berlin"
            autocomplete="off"
            data-testid="session-create-timezone"
          />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm lg:col-span-3">
          <span class="font-medium text-admin-ink">Labels</span>
          <textarea
            class="min-h-28 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs leading-5 text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            placeholder="team=support&#10;purpose=investigation"
            bind:value={draft.labelsText}
            disabled={disabled}
            spellcheck="false"
            data-testid="session-create-labels"
          ></textarea>
          <FieldFeedback errors={validation.fieldErrors.labelsText} hint="One key=value label per line." testId="session-create-labels-error" />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm lg:col-span-3">
          <span class="font-medium text-admin-ink">User agent</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="text"
            bind:value={draft.userAgent}
            disabled={disabled}
            autocomplete="off"
            data-testid="session-create-user-agent"
          />
          <FieldFeedback errors={validation.fieldErrors.userAgent} testId="session-create-user-agent-error" />
        </label>
      </div>
    </section>

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="session-create-payload-section">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">API payload</h4>
      </div>
      <pre class="mt-4 max-h-80 overflow-auto rounded-md border border-admin-border bg-white p-3 text-xs leading-5 text-admin-ink" data-testid="session-create-payload">{validation.preview}</pre>
    </section>
  </div>

  <div class="sticky bottom-0 z-10 -mx-4 -mb-4 mt-5 flex flex-col gap-2 border-t border-admin-border bg-admin-panel/95 px-4 py-3 backdrop-blur sm:-mx-5 sm:-mb-5 sm:flex-row sm:items-center sm:justify-end sm:px-5">
    <button
      class="inline-flex h-10 items-center justify-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink transition hover:bg-admin-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={reset}
      disabled={disabled || !changed}
      data-testid="session-create-reset"
    >
      Reset
    </button>
    <button
      class="inline-flex h-10 items-center justify-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={save}
      disabled={disabled || !validation.valid}
      data-testid="session-create-save"
    >
      Create session
    </button>
  </div>
</section>
