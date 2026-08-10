<script lang="ts">
  import type { ProjectPolicyOptionsLoadState } from '$lib/projects/project-detail-state';
  import type {
    ProjectAllowlistSummary,
    ProjectResourceKind,
  } from '$lib/projects/project-governance-types';
  import { ProjectPolicyEvaluator } from '$lib/projects/project-policy-evaluator';
  import type { ProjectPolicyOptions, ProjectResource } from '$lib/projects/project-types';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';

  type ProjectPolicyEvidenceProps = {
    readonly project: ProjectResource;
    readonly policyOptionsState?: ProjectPolicyOptionsLoadState;
  };

  let {
    project,
    policyOptionsState = { status: 'idle' },
  }: ProjectPolicyEvidenceProps = $props();
  const evaluator = new ProjectPolicyEvaluator();
  const operations = $derived(evaluator.operationPolicies(project));
  const allowlists = $derived(
    policyOptionsState.status === 'ready'
      ? buildAllowlists(policyOptionsState.options)
      : [],
  );

  function buildAllowlists(options: ProjectPolicyOptions): readonly ProjectAllowlistSummary[] {
    const inputs: readonly [ProjectResourceKind, ProjectPolicyOptions[keyof ProjectPolicyOptions]][] = [
      ['session_template', options.sessionTemplates],
      ['browser_context', options.browserContexts],
      ['egress_profile', options.egressProfiles],
      ['extension', options.extensions],
      ['file_workspace', options.fileWorkspaces],
    ];
    return inputs.map(([kind, values]) => evaluator.summarizeAllowlist(project, kind, values));
  }
</script>

<section class="border-b border-admin-border px-4 py-5 sm:px-5" data-testid="project-policy-evidence">
  <div>
    <h4 class="m-0 text-sm font-semibold text-admin-ink">Policy impact</h4>
    <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
      Empty allowlists are unrestricted. Resource scope and lifecycle state are evaluated separately.
    </p>
  </div>

  <div class="mt-4 grid gap-x-5 gap-y-3 sm:grid-cols-2 xl:grid-cols-4">
    {#each operations as operation}
      <div class="border-l-2 border-admin-border pl-3" data-testid={`project-operation-${operation.id}`}>
        <div class="flex flex-wrap items-center gap-2">
          <span class="text-sm font-semibold text-admin-ink">{operation.label}</span>
          <span class={`rounded-full px-2 py-0.5 text-[11px] font-semibold ring-1 ${projectToneClass(operation.tone)}`}>
            {operation.allowed ? 'Allowed' : 'Blocked'}
          </span>
        </div>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">{operation.reason}</p>
      </div>
    {/each}
  </div>

  <div class="mt-5 border-t border-admin-border pt-4">
    <h5 class="m-0 text-sm font-semibold text-admin-ink">Resource allowlists</h5>
    {#if policyOptionsState.status === 'error'}
      <div class="mt-3">
        <AdminMessage
          tone="warning"
          density="compact"
          title="Resource names unavailable"
          message={policyOptionsState.message}
          testId="project-policy-evidence-error"
        />
      </div>
    {:else if policyOptionsState.status !== 'ready'}
      <p class="m-0 mt-3 text-sm text-admin-muted" data-testid="project-policy-evidence-loading">
        Loading policy resources...
      </p>
    {:else}
      <div class="mt-3 grid gap-x-6 gap-y-4 lg:grid-cols-2">
        {#each allowlists as allowlist}
          <section class="min-w-0" aria-label={allowlist.label}>
            <div class="flex flex-wrap items-center gap-2">
              <h6 class="m-0 text-sm font-semibold capitalize text-admin-ink">{allowlist.label}</h6>
              <span class="rounded-full bg-slate-100 px-2 py-0.5 text-[11px] font-semibold text-slate-700">
                {allowlist.mode}
              </span>
            </div>
            {#if allowlist.mode === 'unrestricted'}
              <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">All eligible visible resources may be selected.</p>
            {:else}
              <ul class="m-0 mt-2 grid list-none gap-2 p-0">
                {#each allowlist.resources as resource}
                  <li class="min-w-0 border-l-2 border-admin-border pl-2">
                    <p class="m-0 break-words text-sm font-medium text-admin-ink">{resource.name}</p>
                    <p class={`m-0 mt-0.5 text-xs leading-5 ${resource.allowed ? 'text-admin-muted' : 'text-amber-800'}`}>
                      {resource.reason}
                    </p>
                  </li>
                {/each}
              </ul>
            {/if}
          </section>
        {/each}
      </div>
    {/if}
  </div>
</section>
