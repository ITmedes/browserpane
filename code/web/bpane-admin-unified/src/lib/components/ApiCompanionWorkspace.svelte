<script lang="ts">
  import { ArrowUpRight, FileJson2, ShieldCheck } from '@lucide/svelte';
  import { buildApiTaskFlows } from '$lib/api-companion/api-companion-view-model';
  import type { ApiContractEvidence } from '$lib/api-companion/api-contract-types';
  import AdminMessage from './AdminMessage.svelte';
  import ApiCommandBlock from './ApiCommandBlock.svelte';
  import ApiContractSummary from './ApiContractSummary.svelte';
  import ApiTaskFlowCard from './ApiTaskFlowCard.svelte';

  type ApiCompanionWorkspaceProps = {
    readonly evidence: ApiContractEvidence;
  };

  let { evidence }: ApiCompanionWorkspaceProps = $props();
  const flows = $derived(buildApiTaskFlows(evidence));
  const environmentCommand = `export BPANE_BASE_URL="http://localhost:8080"\nexport BPANE_OWNER_TOKEN="<oidc-owner-access-token>"\nexport BPANE_PROJECT_ID="<project-id>"\nexport BPANE_SESSION_ID="<session-id>"\nexport BPANE_WORKFLOW_ID="<workflow-id>"`;
</script>

<div class="mx-auto grid min-w-0 w-full max-w-[1440px] gap-6 px-4 py-6 sm:px-6" data-testid="api-companion-workspace">
  <header class="flex min-w-0 flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
    <div class="min-w-0 max-w-4xl">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Govern / API reference</p>
      <h1 class="m-0 mt-1 text-2xl font-bold text-admin-ink">Control API companion</h1>
      <p class="m-0 mt-2 text-sm leading-6 text-admin-muted">
        Task-oriented commands derived from the frozen v1 operation inventory and schema-validated examples.
      </p>
    </div>
    <div class="flex flex-wrap gap-2">
      <a class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-semibold text-admin-ink hover:bg-admin-soft" href="/openapi/bpane-control-v1.yaml" download data-testid="api-download-openapi">
        <FileJson2 size={16} strokeWidth={2} />
        Download OpenAPI
      </a>
      <a class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-semibold text-admin-ink hover:bg-admin-soft" href="/admin-new/coverage" data-testid="api-open-coverage">
        Coverage inventory
        <ArrowUpRight size={15} strokeWidth={2} />
      </a>
    </div>
  </header>

  <ApiContractSummary operations={evidence.operations} />

  <AdminMessage
    tone="info"
    title="Credential domains stay separate"
    message="Owner bearer tokens manage control-plane resources. Session connect tickets, session-automation credentials, and recording-worker capabilities are narrower credentials and are not interchangeable. This page never reads or inserts your current browser token."
    testId="api-credential-boundary"
  />

  <section class="grid min-w-0 gap-4 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]" aria-label="API environment setup">
    <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-4 shadow-sm">
      <div class="flex items-start gap-3">
        <span class="inline-flex size-9 shrink-0 items-center justify-center rounded-md bg-admin-soft text-admin-accent"><ShieldCheck size={18} strokeWidth={2} /></span>
        <div class="min-w-0">
          <h2 class="m-0 text-base font-semibold text-admin-ink">Prepare explicit environment values</h2>
          <p class="m-0 mt-1 text-sm leading-6 text-admin-muted">
            Obtain the owner token through your OIDC client flow. Keep it in your process environment or secret store, never in workflow labels or source files.
          </p>
        </div>
      </div>
    </div>
    <ApiCommandBlock command={environmentCommand} label="Shell environment" testId="api-command-environment" />
  </section>

  <section class="grid min-w-0 gap-5" aria-label="API task flows" data-testid="api-task-flows">
    {#each flows as flow (flow.id)}
      <ApiTaskFlowCard {flow} />
    {/each}
  </section>
</div>
