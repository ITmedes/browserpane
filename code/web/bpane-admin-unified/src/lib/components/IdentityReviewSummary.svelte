<script lang="ts">
  import type { IdentityReviewViewModel } from '$lib/identity/identity-review-view-model';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';

  type IdentityReviewSummaryProps = {
    readonly model: IdentityReviewViewModel;
  };

  let { model }: IdentityReviewSummaryProps = $props();
</script>

<div class="grid min-w-0 gap-5" data-testid="identity-review-summary">
  <section class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)]">
    <div class="rounded-md border border-admin-border bg-admin-panel p-4">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Current principal</p>
      <div class="mt-2 flex flex-wrap items-center gap-2">
        <h2 class="m-0 text-lg font-semibold text-admin-ink" data-testid="identity-principal-name">
          {model.principalName}
        </h2>
        <span class="rounded-full bg-blue-50 px-2 py-0.5 text-xs font-semibold text-blue-700 ring-1 ring-blue-200">
          {model.principalType}
        </span>
      </div>
      <dl class="m-0 mt-4 grid gap-3">
        {#each model.principalFacts as fact}
          <div class="grid min-w-0 gap-1">
            <dt class="text-xs font-semibold text-admin-muted">{fact.label}</dt>
            <dd class="m-0 break-all font-mono text-xs text-admin-ink">{fact.value}</dd>
          </div>
        {/each}
      </dl>
      <p class="m-0 mt-4 text-xs text-admin-muted">Review generated {model.generatedAt}</p>
    </div>

    <div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3" aria-label="Identity resource counts">
      {#each model.metrics as metric}
        <div class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid={`identity-metric-${metric.id}`}>
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</p>
          <p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p>
        </div>
      {/each}
    </div>
  </section>

  <AdminMessage
    tone="info"
    title="Review evidence, not complete RBAC"
    message="Current scopes, allowed projects, and mappings are sanitized registry and access-review metadata. Enforced organization and project grants remain a separate control-plane capability."
    testId="identity-enforcement-boundary"
  />

  <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="identity-project-access">
    <div class="border-b border-admin-border px-4 py-3">
      <h2 class="m-0 text-sm font-semibold text-admin-ink">Project access review</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">Owner-visible project summaries and current usage evidence.</p>
    </div>
    <div class="overflow-auto">
      <table class="w-full min-w-[760px] border-collapse">
        <thead class="bg-admin-soft">
          <tr class="border-b border-admin-border">
            <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Project</th>
            <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">State</th>
            <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Sessions</th>
            <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Workflow runs</th>
            <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Alerts</th>
          </tr>
        </thead>
        <tbody>
          {#if model.projects.length === 0}
            <tr><td class="px-4 py-10 text-center text-sm text-admin-muted" colspan="5">No projects are visible.</td></tr>
          {:else}
            {#each model.projects as project}
              <tr class="border-b border-admin-border last:border-b-0" data-testid="identity-project-row">
                <td class="px-4 py-3">
                  <a class="text-sm font-semibold text-admin-ink hover:text-admin-accent" href={`/admin-new/projects/${encodeURIComponent(project.id)}`}>
                    {project.name}
                  </a>
                  <p class="m-0 mt-1 font-mono text-[11px] text-admin-muted">{project.id}</p>
                </td>
                <td class="px-3 py-3"><span class={`rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(project.stateTone)}`}>{project.state}</span></td>
                <td class="px-3 py-3 text-xs text-admin-muted">{project.sessions}</td>
                <td class="px-3 py-3 text-xs text-admin-muted">{project.workflowRuns}</td>
                <td class="px-4 py-3 text-xs text-admin-muted">{project.alerts}</td>
              </tr>
            {/each}
          {/if}
        </tbody>
      </table>
    </div>
  </section>

  <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="identity-delegations">
    <div class="border-b border-admin-border px-4 py-3">
      <h2 class="m-0 text-sm font-semibold text-admin-ink">Delegated automation principals</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">Safe client identity and session-correlation evidence.</p>
    </div>
    {#if model.delegations.length === 0}
      <p class="m-0 px-4 py-10 text-center text-sm text-admin-muted">No automation principals are delegated.</p>
    {:else}
      <div class="grid divide-y divide-admin-border">
        {#each model.delegations as delegation}
          <article class="grid min-w-0 gap-3 px-4 py-3 lg:grid-cols-[minmax(0,1fr)_auto_auto] lg:items-center" data-testid="identity-delegation-row">
            <div class="min-w-0">
              <p class="m-0 truncate text-sm font-semibold text-admin-ink">{delegation.name}</p>
              <p class="m-0 mt-1 truncate font-mono text-xs text-admin-muted">{delegation.clientId}</p>
              <p class="m-0 mt-1 break-all text-xs text-admin-muted">{delegation.issuer}</p>
            </div>
            <div class="flex flex-wrap gap-2">
              <span class={`rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(delegation.registrationTone)}`}>{delegation.registration}</span>
              <span class={`rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(delegation.stateTone)}`}>{delegation.state}</span>
            </div>
            <div class="min-w-0 text-xs text-admin-muted lg:text-right">
              <p class="m-0 font-semibold text-admin-ink">{delegation.sessions}</p>
              <p class="m-0 mt-1 max-w-md break-all font-mono text-[11px]">{delegation.sessionIds}</p>
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </section>

  <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="identity-unmapped-signals">
    <div class="border-b border-admin-border px-4 py-3">
      <h2 class="m-0 text-sm font-semibold text-admin-ink">Unmapped safe signals</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">Allowlisted identity evidence that has no active project mapping.</p>
    </div>
    {#if model.unmappedSignals.length === 0}
      <p class="m-0 px-4 py-10 text-center text-sm text-admin-muted">No unmapped safe identity signals.</p>
    {:else}
      <div class="grid divide-y divide-admin-border">
        {#each model.unmappedSignals as signal}
          <article class="grid min-w-0 gap-2 px-4 py-3 md:grid-cols-[120px_minmax(0,1fr)_minmax(0,1fr)]" data-testid="identity-unmapped-row">
            <span class="text-xs font-semibold text-admin-ink">{signal.kind}</span>
            <div class="min-w-0">
              <p class="m-0 break-all font-mono text-xs text-admin-ink">{signal.identity}</p>
              <p class="m-0 mt-1 break-all text-xs text-admin-muted">{signal.issuer}</p>
            </div>
            <p class="m-0 text-xs leading-5 text-admin-muted">{signal.reason}</p>
          </article>
        {/each}
      </div>
    {/if}
  </section>
</div>
