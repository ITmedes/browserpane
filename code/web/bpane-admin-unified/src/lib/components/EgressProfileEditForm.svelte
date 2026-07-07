<script lang="ts">
  import {
    buildEgressProfileStatusSummaryModel,
    createEgressProfileEditDraft,
    createNewEgressProfileEditDraft,
    hasEgressProfileEditChanges,
    hasNewEgressProfileEditChanges,
    mergeProjectsWithSelected,
    setEgressObservationMode,
    validateEgressProfileEdit,
    type EgressProfileEditDraft,
    type EgressProfileEditMode,
  } from '$lib/egress-profiles/egress-profile-edit-view-model';
  import type { EgressProfileProjectOptionsLoadState } from '$lib/egress-profiles/egress-profile-detail-state';
  import type { EgressProfileResource, UpsertEgressProfileRequest } from '$lib/egress-profiles/egress-profile-types';
  import AdminMessage from './AdminMessage.svelte';
  import EgressProfileStatusSummary from './EgressProfileStatusSummary.svelte';
  import FieldFeedback from './FieldFeedback.svelte';

  type EgressProfileEditFormProps = {
    readonly profile?: EgressProfileResource;
    readonly mode?: EgressProfileEditMode;
    readonly disabled?: boolean;
    readonly projectOptionsState?: EgressProfileProjectOptionsLoadState;
    readonly onSave?: (request: UpsertEgressProfileRequest) => void | Promise<void>;
  };

  let {
    profile,
    mode = profile ? 'edit' : 'create',
    disabled = false,
    projectOptionsState = { status: 'idle' },
    onSave,
  }: EgressProfileEditFormProps = $props();
  // svelte-ignore state_referenced_locally
  let draft = $state<EgressProfileEditDraft>(initialDraft());

  const validation = $derived(validateEgressProfileEdit(profile ?? null, draft));
  const createMode = $derived(mode === 'create');
  const changed = $derived(createMode
    ? hasNewEgressProfileEditChanges(draft)
    : profile ? hasEgressProfileEditChanges(profile, draft) : false);
  const statusModel = $derived(profile ? buildEgressProfileStatusSummaryModel(profile) : null);
  const title = $derived(createMode ? 'New egress profile settings' : 'Egress profile settings');
  const description = $derived(createMode
    ? 'Define scope, proxy, TLS interception, and diagnostics posture before creating the profile.'
    : 'Review current status and update scope, proxy, TLS interception, and diagnostics posture.');
  const changeStatusLabel = $derived(disabled
    ? 'Read-only access'
    : createMode
      ? validation.valid
        ? 'Ready to create'
        : 'Profile draft'
      : changed
        ? 'Unsaved changes'
        : 'No changes');
  const changeStatusClass = $derived(disabled
    ? 'border-admin-border bg-admin-soft text-admin-muted'
    : createMode
      ? validation.valid
        ? 'border-emerald-200 bg-emerald-50 text-emerald-800'
        : 'border-amber-200 bg-amber-50 text-amber-900'
      : changed
        ? 'border-amber-200 bg-amber-50 text-amber-900'
        : 'border-emerald-200 bg-emerald-50 text-emerald-800');
  const saveLabel = $derived(createMode ? 'Create egress profile' : 'Save changes');
  const tlsIntercept = $derived(draft.observationMode === 'tls_intercept');
  const projectOptions = $derived(projectOptionsState.status === 'ready'
    ? mergeProjectsWithSelected(projectOptionsState.projects, draft.projectId)
    : mergeProjectsWithSelected([], draft.projectId));

  function initialDraft(): EgressProfileEditDraft {
    return profile ? createEgressProfileEditDraft(profile) : createNewEgressProfileEditDraft();
  }

  function reset(): void {
    draft = initialDraft();
  }

  function save(): void {
    if (!validation.request) {
      return;
    }
    void onSave?.(validation.request);
  }

  function checked(event: Event): boolean {
    return (event.currentTarget as HTMLInputElement).checked;
  }

  function value(event: Event): string {
    return (event.currentTarget as HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement).value;
  }
</script>

<section class="rounded-md border border-admin-border bg-admin-panel p-4 sm:p-5" data-testid="egress-profile-edit-form">
  <div class="flex flex-col gap-3 border-b border-admin-border pb-4 lg:flex-row lg:items-start lg:justify-between">
    <div class="min-w-0">
      <h3 class="m-0 text-base font-semibold text-admin-ink">{title}</h3>
      <p class="m-0 mt-1 text-sm leading-6 text-admin-muted">{description}</p>
    </div>
    <span class={`inline-flex w-fit shrink-0 rounded-full border px-2.5 py-1 text-xs font-semibold ${changeStatusClass}`}>
      {changeStatusLabel}
    </span>
  </div>

  <div class="mt-5 grid gap-4">
    {#if statusModel}
      <EgressProfileStatusSummary model={statusModel} />
    {/if}

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="egress-profile-metadata-editor">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Metadata</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Keep the profile identity and lifecycle state clear for operators selecting session egress.
        </p>
      </div>

      <div class="mt-4 grid items-start gap-4 lg:grid-cols-[minmax(0,1fr)_220px]">
        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Name</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="text"
            bind:value={draft.name}
            disabled={disabled}
            autocomplete="off"
            data-testid="egress-profile-edit-name"
          />
          <FieldFeedback errors={validation.fieldErrors.name} testId="egress-profile-edit-name-error" />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">State</span>
          <select
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.state}
            disabled={disabled}
            data-testid="egress-profile-edit-state"
          >
            <option value="ready">ready</option>
            <option value="disabled">disabled</option>
          </select>
          <FieldFeedback hint="Disabled profiles remain visible but cannot be selected for healthy launches." />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm lg:col-span-2">
          <span class="font-medium text-admin-ink">Description</span>
          <textarea
            class="min-h-24 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 py-2 text-sm leading-6 text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.description}
            disabled={disabled}
            data-testid="egress-profile-edit-description"
          ></textarea>
          <FieldFeedback hint="Optional operator-facing explanation for this egress profile." />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm lg:col-span-2">
          <span class="font-medium text-admin-ink">Labels</span>
          <textarea
            class="min-h-28 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs leading-5 text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            placeholder="team=support&#10;region=eu"
            bind:value={draft.labelsText}
            disabled={disabled}
            spellcheck="false"
            data-testid="egress-profile-edit-labels"
          ></textarea>
          <FieldFeedback
            errors={validation.fieldErrors.labels}
            hint="One key=value label per line."
            testId="egress-profile-edit-labels-error"
          />
        </label>
      </div>
    </section>

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="egress-profile-scope-editor">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Scope</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Owner-scoped profiles are reusable across projects. Project-scoped profiles are only usable by matching sessions.
        </p>
      </div>

      {#if projectOptionsState.status === 'loading'}
        <div class="mt-4">
          <AdminMessage tone="loading" title="Loading projects" testId="egress-profile-projects-loading" />
        </div>
      {:else if projectOptionsState.status === 'error'}
        <div class="mt-4">
          <AdminMessage
            tone="warning"
            title="Project options unavailable"
            message={projectOptionsState.message}
            testId="egress-profile-projects-error"
          />
        </div>
      {/if}

      <div class="mt-4 grid items-start gap-4 lg:grid-cols-[220px_minmax(0,1fr)]">
        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Binding</span>
          <select
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.projectBinding}
            disabled={disabled}
            data-testid="egress-profile-edit-project-binding"
          >
            <option value="owner">owner scoped</option>
            <option value="project">project scoped</option>
          </select>
          <FieldFeedback hint="Choose whether this profile is reusable globally or bound to one project." />
        </label>

        {#if draft.projectBinding === 'project'}
          <label class="grid min-w-0 content-start gap-1.5 text-sm">
            <span class="font-medium text-admin-ink">Project</span>
            <select
              class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
              bind:value={draft.projectId}
              disabled={disabled}
              data-testid="egress-profile-edit-project-id"
            >
              <option value="">Select a project...</option>
              {#each projectOptions as project}
                <option value={project.id}>{project.name} · {project.state}</option>
              {/each}
            </select>
            <FieldFeedback errors={validation.fieldErrors.projectId} testId="egress-profile-edit-project-id-error" />
          </label>
        {:else}
          <p class="m-0 self-start rounded-md bg-admin-soft px-3 py-2 text-xs leading-5 text-admin-muted lg:mt-6">
            The API will persist `project_id=null`.
          </p>
        {/if}
      </div>
    </section>

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="egress-profile-proxy-editor">
      <div class="flex flex-col gap-3 border-b border-admin-border pb-3 sm:flex-row sm:items-start sm:justify-between">
        <div class="min-w-0">
          <h4 class="m-0 text-sm font-semibold text-admin-ink">Proxy</h4>
          <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
            Configure a forward proxy and optional secret-backed credential binding for outbound browser traffic.
          </p>
        </div>
        <label class="inline-flex shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-2.5 py-1.5 text-sm font-medium text-admin-ink">
          <input
            class="h-4 w-4 rounded border-admin-border text-admin-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 disabled:cursor-not-allowed disabled:opacity-60"
            type="checkbox"
            checked={draft.proxyEnabled}
            disabled={disabled || tlsIntercept}
            onchange={(event) => {
              draft = { ...draft, proxyEnabled: checked(event) };
            }}
            data-testid="egress-profile-edit-proxy-enabled"
          />
          <span>Use proxy</span>
        </label>
      </div>

      {#if draft.proxyEnabled}
        <div class="mt-4 grid items-start gap-4 lg:grid-cols-2">
          <label class="grid min-w-0 content-start gap-1.5 text-sm">
            <span class="font-medium text-admin-ink">Proxy URL</span>
            <input
              class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
              type="url"
              placeholder="http://proxy.example:3128"
              bind:value={draft.proxyUrl}
              disabled={disabled}
              data-testid="egress-profile-edit-proxy-url"
            />
            <FieldFeedback errors={validation.fieldErrors.proxyUrl} testId="egress-profile-edit-proxy-url-error" />
          </label>

          <label class="grid min-w-0 content-start gap-1.5 text-sm">
            <span class="font-medium text-admin-ink">Credential binding id</span>
            <input
              class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
              type="text"
              placeholder="optional UUID"
              bind:value={draft.proxyCredentialBindingId}
              disabled={disabled}
              data-testid="egress-profile-edit-proxy-credential-binding-id"
            />
            <FieldFeedback
              errors={validation.fieldErrors.proxyCredentialBindingId}
              hint="Optional UUID of a secret-backed credential binding."
              testId="egress-profile-edit-proxy-credential-binding-id-error"
            />
          </label>

          <label class="grid min-w-0 content-start gap-1.5 text-sm lg:col-span-2">
            <span class="font-medium text-admin-ink">Bypass rules</span>
            <textarea
              class="min-h-24 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs leading-5 text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
              placeholder="localhost&#10;127.0.0.1"
              bind:value={draft.bypassRulesText}
              disabled={disabled}
              spellcheck="false"
              data-testid="egress-profile-edit-bypass-rules"
            ></textarea>
            <FieldFeedback
              errors={validation.fieldErrors.bypassRules}
              hint="One bypass host or pattern per line."
              testId="egress-profile-edit-bypass-rules-error"
            />
          </label>
        </div>
      {:else}
        <p class="m-0 mt-4 rounded-md bg-admin-soft px-3 py-2 text-xs text-admin-muted">
          Direct egress. The API will persist `proxy=null`.
        </p>
      {/if}
    </section>

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="egress-profile-ca-editor">
      <div class="flex flex-col gap-3 border-b border-admin-border pb-3 sm:flex-row sm:items-start sm:justify-between">
        <div class="min-w-0">
          <h4 class="m-0 text-sm font-semibold text-admin-ink">Custom CA</h4>
          <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
            Attach a custom CA reference when browser runtimes must trust a TLS interception proxy.
          </p>
        </div>
        <label class="inline-flex shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-2.5 py-1.5 text-sm font-medium text-admin-ink">
          <input
            class="h-4 w-4 rounded border-admin-border text-admin-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 disabled:cursor-not-allowed disabled:opacity-60"
            type="checkbox"
            checked={draft.customCaEnabled}
            disabled={disabled || tlsIntercept}
            onchange={(event) => {
              draft = { ...draft, customCaEnabled: checked(event) };
            }}
            data-testid="egress-profile-edit-custom-ca-enabled"
          />
          <span>Attach CA</span>
        </label>
      </div>

      {#if draft.customCaEnabled}
        <div class="mt-4 grid items-start gap-4 lg:grid-cols-2">
          <label class="grid min-w-0 content-start gap-1.5 text-sm">
            <span class="font-medium text-admin-ink">Certificate reference</span>
            <input
              class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
              type="text"
              placeholder="file:///workspace/dev/egress-ca.pem"
              bind:value={draft.customCaCertificateRef}
              disabled={disabled}
              data-testid="egress-profile-edit-custom-ca-certificate-ref"
            />
            <FieldFeedback
              errors={validation.fieldErrors.customCaCertificateRef}
              testId="egress-profile-edit-custom-ca-certificate-ref-error"
            />
          </label>

          <label class="grid min-w-0 content-start gap-1.5 text-sm">
            <span class="font-medium text-admin-ink">Display name</span>
            <input
              class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
              type="text"
              placeholder="Local intercept CA"
              bind:value={draft.customCaDisplayName}
              disabled={disabled}
              data-testid="egress-profile-edit-custom-ca-display-name"
            />
            <FieldFeedback
              errors={validation.fieldErrors.customCaDisplayName}
              hint="Optional display name shown to operators."
              testId="egress-profile-edit-custom-ca-display-name-error"
            />
          </label>
        </div>
      {:else}
        <p class="m-0 mt-4 rounded-md bg-admin-soft px-3 py-2 text-xs text-admin-muted">
          No custom CA will be materialized into browser runtimes.
        </p>
      {/if}
    </section>

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="egress-profile-observation-editor">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Traffic observation</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Metadata-only profiles rely on the proxy logs. TLS interception requires explicit proxy, CA, and sensitive log sink references.
        </p>
      </div>

      <div class="mt-4 grid items-start gap-4 lg:grid-cols-2">
        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Observation mode</span>
          <select
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            value={draft.observationMode}
            disabled={disabled}
            onchange={(event) => {
              draft = setEgressObservationMode(draft, value(event) as EgressProfileEditDraft['observationMode']);
            }}
            data-testid="egress-profile-edit-observation-mode"
          >
            <option value="metadata_only">metadata_only</option>
            <option value="tls_intercept">tls_intercept</option>
          </select>
          <FieldFeedback hint="TLS interception automatically requires proxy and custom CA settings." />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Sensitive log sink reference</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="text"
            placeholder="siem://browserpane/support"
            bind:value={draft.sensitiveLogSinkRef}
            disabled={disabled}
            data-testid="egress-profile-edit-sensitive-log-sink-ref"
          />
          <FieldFeedback
            errors={validation.fieldErrors.sensitiveLogSinkRef}
            testId="egress-profile-edit-sensitive-log-sink-ref-error"
          />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm lg:col-span-2">
          <span class="font-medium text-admin-ink">Sensitive log sink display name</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="text"
            placeholder="Support SIEM"
            bind:value={draft.sensitiveLogSinkDisplayName}
            disabled={disabled}
            data-testid="egress-profile-edit-sensitive-log-sink-display-name"
          />
          <FieldFeedback
            errors={validation.fieldErrors.sensitiveLogSinkDisplayName}
            hint="Optional label for the approved sensitive log sink."
            testId="egress-profile-edit-sensitive-log-sink-display-name-error"
          />
        </label>
      </div>

      {#if tlsIntercept}
        <div class="mt-4">
          <AdminMessage
            tone="warning"
            density="compact"
            title="TLS intercept requirements"
            message="This mode requires a proxy URL, custom CA reference, and approved sensitive log sink reference."
            testId="egress-profile-edit-tls-requirements"
          />
        </div>
      {/if}
    </section>
  </div>

  <div class="sticky bottom-0 z-10 -mx-4 -mb-4 mt-5 flex flex-col gap-2 border-t border-admin-border bg-admin-panel/95 px-4 py-3 backdrop-blur sm:-mx-5 sm:-mb-5 sm:flex-row sm:items-center sm:justify-end sm:px-5">
    <button
      class="inline-flex h-10 w-full items-center justify-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink transition hover:bg-admin-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60 sm:w-auto"
      type="button"
      onclick={reset}
      disabled={disabled || !changed}
      data-testid="egress-profile-edit-cancel"
    >
      Cancel
    </button>
    <button
      class="inline-flex h-10 w-full items-center justify-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-60 sm:w-auto"
      type="button"
      onclick={save}
      disabled={disabled || !changed || !validation.valid}
      data-testid="egress-profile-edit-save"
    >
      {saveLabel}
    </button>
  </div>
</section>
