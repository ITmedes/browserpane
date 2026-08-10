<script lang="ts">
  import { SessionCreateGovernancePresenter } from '$lib/sessions/session-create-governance';
  import type {
    SessionCreateDraft,
    SessionCreateOptions,
    SessionCreateValidationResult,
  } from '$lib/sessions/session-create-view-model';
  import FieldFeedback from './FieldFeedback.svelte';
  import SessionCreateProjectEvidence from './SessionCreateProjectEvidence.svelte';

  type SessionCreateScopeFieldsProps = {
    readonly draft: SessionCreateDraft;
    readonly options: SessionCreateOptions;
    readonly validation: SessionCreateValidationResult;
    readonly disabled?: boolean;
    readonly onDraftChange?: (draft: SessionCreateDraft) => void;
  };

  let {
    draft,
    options,
    validation,
    disabled = false,
    onDraftChange,
  }: SessionCreateScopeFieldsProps = $props();
  const presenter = new SessionCreateGovernancePresenter();
  const governance = $derived(presenter.build(draft.projectId, options));

  function update<K extends keyof SessionCreateDraft>(key: K, value: SessionCreateDraft[K]): void {
    onDraftChange?.({ ...draft, [key]: value });
  }

  function selectedValue(event: Event): string {
    return (event.currentTarget as HTMLSelectElement).value;
  }

  function choiceLabel(choice: (typeof governance.sessionTemplates)[number]): string {
    const metadata = [choice.state, choice.scope].filter(Boolean).join(' / ');
    const base = metadata ? `${choice.name} - ${metadata}` : choice.name;
    return choice.disabled ? `${base} - unavailable: ${choice.reason}` : base;
  }
</script>

<section class="min-w-0 rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="session-create-scope-section">
  <div class="border-b border-admin-border pb-3">
    <h4 class="m-0 text-sm font-semibold text-admin-ink">Scope and governed resources</h4>
    <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
      Select a project to preview current pressure and apply its resource allowlists before submission.
    </p>
  </div>

  <div class="mt-4 grid items-start gap-4 lg:grid-cols-3">
    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Project</span>
      <select
        class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
        value={draft.projectId}
        onchange={(event) => update('projectId', selectedValue(event))}
        {disabled}
        data-testid="session-create-project-id"
      >
        <option value="">Owner scoped</option>
        {#each options.projects as project}
          <option value={project.id} disabled={project.state === 'archived'}>{project.name} - {project.state}</option>
        {/each}
      </select>
      <FieldFeedback errors={validation.fieldErrors.projectId} testId="session-create-project-id-error" />
    </label>

    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Session template</span>
      <select
        class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
        value={draft.templateId}
        onchange={(event) => update('templateId', selectedValue(event))}
        {disabled}
        data-testid="session-create-template-id"
      >
        <option value="">No template</option>
        {#each governance.sessionTemplates as template}
          <option value={template.id} disabled={template.disabled}>{choiceLabel(template)}</option>
        {/each}
      </select>
      <FieldFeedback errors={validation.fieldErrors.templateId} testId="session-create-template-id-error" />
    </label>

    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Owner mode</span>
      <select
        class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
        value={draft.ownerMode}
        onchange={(event) => update('ownerMode', selectedValue(event) as SessionCreateDraft['ownerMode'])}
        {disabled}
        data-testid="session-create-owner-mode"
      >
        <option value="">Gateway default</option>
        <option value="collaborative">collaborative</option>
        <option value="exclusive_browser_owner">exclusive browser owner</option>
      </select>
    </label>

    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Browser context mode</span>
      <select
        class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
        value={draft.browserContextMode}
        onchange={(event) => update('browserContextMode', selectedValue(event) as SessionCreateDraft['browserContextMode'])}
        {disabled}
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
        value={draft.browserContextId}
        onchange={(event) => update('browserContextId', selectedValue(event))}
        disabled={disabled || draft.browserContextMode !== 'reusable'}
        data-testid="session-create-browser-context-id"
      >
        <option value="">Select context</option>
        {#each governance.browserContexts as context}
          <option value={context.id} disabled={context.disabled}>{choiceLabel(context)}</option>
        {/each}
      </select>
      <FieldFeedback errors={validation.fieldErrors.browserContextId} testId="session-create-browser-context-id-error" />
    </label>

    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Egress profile</span>
      <select
        class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
        value={draft.egressProfileId}
        onchange={(event) => update('egressProfileId', selectedValue(event))}
        {disabled}
        data-testid="session-create-egress-profile-id"
      >
        <option value="">No explicit egress profile</option>
        {#each governance.egressProfiles as profile}
          <option value={profile.id} disabled={profile.disabled}>{choiceLabel(profile)}</option>
        {/each}
      </select>
      <FieldFeedback errors={validation.fieldErrors.egressProfileId} testId="session-create-egress-profile-id-error" />
    </label>
  </div>

  {#if governance.project}
    <SessionCreateProjectEvidence project={governance.project} />
  {/if}
</section>
